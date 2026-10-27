//! The program's allocator: caches of small blocks over std's (dlmalloc behind one spin lock,
//! which every allocation of every thread would otherwise take in turn). A thread takes that lock
//! once per batch of blocks; larger blocks go to dlmalloc as before.
//!
//! The caches are shards picked by the stack's address: each thread's stack is its own 8 MB of
//! memory (web/www/threads.js), so threads keep to their own shard and its lock stays free.
//! Thread-locals can't serve: a new worker allocates its stack and its thread-local storage here
//! before that storage exists.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

/// Block sizes, each a multiple of 16 (the largest alignment cached).
const SIZES: [usize; 14] = [16, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048];
const LARGEST: usize = 2048;
const ALIGN: usize = 16;
/// Blocks a refill brings, and the most a list keeps before it gives half back.
const BATCH: usize = 32;
const KEEP: usize = 4 * BATCH;
const SHARDS: usize = 64;
/// A stack's size, as a power of two: the bits above pick the shard.
const STACK_BITS: u32 = 23;

pub struct Allocator;

struct List {
    head: *mut u8,
    len: usize,
}

struct Shard {
    locked: AtomicBool,
    lists: UnsafeCell<[List; SIZES.len()]>,
}

// Safety: `lists` is only touched by whoever holds `locked`.
unsafe impl Sync for Shard {}

static CACHES: [Shard; SHARDS] = [const {
    Shard {
        locked: AtomicBool::new(false),
        lists: UnsafeCell::new([const { List { head: ptr::null_mut(), len: 0 } }; SIZES.len()]),
    }
}; SHARDS];

struct Held(&'static Shard);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.locked.store(false, Ordering::Release);
    }
}

/// This thread's shard, or the first free one after it; `None` when all are held.
fn lock() -> Option<Held> {
    let marker = 0u8;
    let stack = std::hint::black_box(&marker) as *const u8 as usize;
    let first = (stack >> STACK_BITS) % SHARDS;
    (0..SHARDS).map(|i| &CACHES[(first + i) % SHARDS]).find_map(|shard| {
        // `then`, not `then_some`: a guard for a shard someone else holds would unlock it on drop.
        (!shard.locked.swap(true, Ordering::Acquire)).then(|| Held(shard))
    })
}

fn class(layout: Layout) -> Option<usize> {
    if layout.size() > LARGEST || layout.align() > ALIGN {
        return None;
    }
    SIZES.iter().position(|&size| size >= layout.size().max(1))
}

fn block(class: usize) -> Layout {
    // Every size is a non-zero multiple of 16.
    unsafe { Layout::from_size_align_unchecked(SIZES[class], ALIGN) }
}

// Safety: the free lists hold blocks of their class only, each with the next block's address in
// its first bytes. A block can be freed into another shard than it came from: it is the same size.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(class) = class(layout) else {
            return unsafe { System.alloc(layout) };
        };
        // A whole block, as it may go to a list when freed.
        let Some(held) = lock() else {
            return unsafe { System.alloc(block(class)) };
        };
        let list = unsafe { &mut (*held.0.lists.get())[class] };
        if list.head.is_null() {
            for _ in 0..BATCH {
                let fresh = unsafe { System.alloc(block(class)) };
                if fresh.is_null() {
                    break;
                }
                unsafe { fresh.cast::<*mut u8>().write(list.head) };
                list.head = fresh;
                list.len += 1;
            }
            if list.head.is_null() {
                return ptr::null_mut();
            }
        }
        let taken = list.head;
        list.head = unsafe { taken.cast::<*mut u8>().read() };
        list.len -= 1;
        taken
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(class) = class(layout) else {
            return unsafe { System.dealloc(ptr, layout) };
        };
        let Some(held) = lock() else {
            return unsafe { System.dealloc(ptr, block(class)) };
        };
        let list = unsafe { &mut (*held.0.lists.get())[class] };
        unsafe { ptr.cast::<*mut u8>().write(list.head) };
        list.head = ptr;
        list.len += 1;
        if list.len > KEEP {
            for _ in 0..KEEP / 2 {
                let given = list.head;
                list.head = unsafe { given.cast::<*mut u8>().read() };
                list.len -= 1;
                unsafe { System.dealloc(given, block(class)) };
            }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if class(layout).is_none() {
            return unsafe { System.alloc_zeroed(layout) };
        }
        let ptr = unsafe { self.alloc(layout) };
        if !ptr.is_null() {
            unsafe { ptr.write_bytes(0, layout.size()) };
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (class(layout), class(new)) {
            (None, None) => unsafe { System.realloc(ptr, layout, new_size) },
            (Some(a), Some(b)) if a == b => ptr,
            _ => {
                let moved = unsafe { self.alloc(new) };
                if !moved.is_null() {
                    unsafe { ptr::copy_nonoverlapping(ptr, moved, layout.size().min(new_size)) };
                    unsafe { self.dealloc(ptr, layout) };
                }
                moved
            }
        }
    }
}
