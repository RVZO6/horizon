//! The browser build's platform layer. `web/std` patches Rust's standard library for
//! wasm32-unknown-unknown with atomics; this crate defines the `__horizon_web_*` functions that
//! patch calls, and the entry points `web/www/main.js` and `web/www/worker.js` call.
//!
//! Threads: a new Web Worker only starts once the thread that made it returns to its event loop,
//! and the game's main thread spins (it may not block) while it waits for threads it started. So
//! the page starts a pool of workers ahead of time; each one waits on shared memory for the next
//! `std::thread::spawn` and runs it, and a worker that never blocks (`web/www/spawner.js`) starts
//! a replacement for every worker taken.
#![cfg(target_arch = "wasm32")]

mod alloc;
mod audio;

pub use audio::AudioOutput;

#[global_allocator]
static ALLOCATOR: alloc::Allocator = alloc::Allocator;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function now() {
    return performance.timeOrigin + performance.now();
}
export function parallelism() {
    return navigator.hardwareConcurrency || 4;
}
export function print(fd, text) {
    if (fd === 2) console.warn(text); else console.log(text);
}
// Files live under fs/ beside the page (and the workers' script).
export function fetch_sync(path) {
    const xhr = new XMLHttpRequest();
    xhr.open('GET', new URL('fs' + path, self.location.href), false);
    const window_thread = typeof document !== 'undefined';
    // A window may not ask a synchronous request for an ArrayBuffer: read bytes as text.
    if (window_thread) xhr.overrideMimeType('text/plain; charset=x-user-defined');
    else xhr.responseType = 'arraybuffer';
    try {
        xhr.send();
    } catch (e) {
        console.error('horizon-web: fetching', path, e);
        return undefined;
    }
    if (xhr.status !== 200) {
        console.error('horizon-web: fetching', path, 'HTTP', xhr.status);
        return undefined;
    }
    if (!window_thread) return new Uint8Array(xhr.response);
    const text = xhr.responseText;
    const out = new Uint8Array(text.length);
    for (let i = 0; i < text.length; i++) out[i] = text.charCodeAt(i) & 0xff;
    return out;
}
export function loading(progress, ready) {
    self.dispatchEvent(new CustomEvent('horizon-loading', { detail: { progress, ready } }));
}
// The frame on screen as a PNG download (`save_canvas`).
export function save_canvas(name) {
    const canvas = document.querySelector('canvas:not(.horizon-overlay)');
    canvas?.toBlob((blob) => {
        if (!blob) return;
        const link = document.createElement('a');
        link.href = URL.createObjectURL(blob);
        link.download = name;
        link.click();
        setTimeout(() => URL.revokeObjectURL(link.href), 10000);
    }, 'image/png');
}
// web/www/spawner.js starts a replacement.
export function worker_taken() {
    self.horizonThreads ??= new BroadcastChannel(self.horizonChannel);
    self.horizonThreads.postMessage('taken');
}
"#)]
extern "C" {
    fn now() -> f64;
    fn parallelism() -> u32;
    fn print(fd: u32, text: &str);
    fn fetch_sync(path: &str) -> Option<js_sys::Uint8Array>;
    fn worker_taken();
    #[wasm_bindgen(js_name = loading)]
    fn loading_event(progress: f32, ready: bool);
    #[wasm_bindgen(js_name = save_canvas)]
    fn save_canvas_js(name: &str);
}

/// F2: the game's canvas, as it is on screen, downloaded as a PNG named `name`.
pub fn save_canvas(name: &str) {
    save_canvas_js(name);
}

static WANTS_POINTER_LOCK: AtomicBool = AtomicBool::new(false);
static POINTER_LOCKED: AtomicBool = AtomicBool::new(false);

/// The game captures the mouse (or lets it go). A browser locks the pointer only on a click, so the
/// page asks on the next one (web/www/main.js), as vanilla grabs the mouse on a click.
pub fn want_pointer_lock(want: bool) {
    WANTS_POINTER_LOCK.store(want, Ordering::Relaxed);
}

#[wasm_bindgen]
pub fn horizon_web_wants_pointer_lock() -> bool {
    WANTS_POINTER_LOCK.load(Ordering::Relaxed)
}

/// Whether the game captures the mouse (`want_pointer_lock`).
pub fn wants_pointer_lock() -> bool {
    WANTS_POINTER_LOCK.load(Ordering::Relaxed)
}

/// The browser locked the game's pointer, or let it go (Escape, another tab).
#[wasm_bindgen]
pub fn horizon_web_pointer_lock_changed(locked: bool) {
    POINTER_LOCKED.store(locked, Ordering::Relaxed);
}

/// Whether the browser holds the pointer for the game: only then is the mouse captured.
pub fn pointer_locked() -> bool {
    POINTER_LOCKED.load(Ordering::Relaxed)
}

static LOADED: AtomicBool = AtomicBool::new(false);

/// The game's loading, for the page's Mojang Studios screen (web/www/overlay.js): how far the
/// resources are, and whether the game has a screen to show (the overlay's reload is done).
/// Nothing after that.
pub fn loading(progress: f32, ready: bool) {
    if LOADED.load(Ordering::Relaxed) {
        return;
    }
    LOADED.store(ready, Ordering::Relaxed);
    loading_event(progress, ready);
}

unsafe extern "C" {
    fn __horizon_std_set_can_block(can_block: bool);
    fn __horizon_std_thread_main(packet: *mut u8);
    fn __horizon_std_push_arg(ptr: *const u8, len: usize);
    fn __horizon_std_memfs_lazy(ptr: *const u8, len: usize, size: u64);
    fn __horizon_std_memfs_insert(ptr: *const u8, len: usize, data: *mut u8);
    fn __horizon_std_failure_flag() -> *const u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn __horizon_web_now() -> f64 {
    now()
}

#[unsafe(no_mangle)]
pub extern "C" fn __horizon_web_parallelism() -> u32 {
    parallelism()
}

thread_local! {
    static LINE: [RefCell<Vec<u8>>; 2] = const { [RefCell::new(Vec::new()), RefCell::new(Vec::new())] };
}

/// Console output, one call per line.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __horizon_web_print(fd: u32, ptr: *const u8, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let index = usize::from(fd == 2);
    LINE.with(|lines| {
        let mut line = lines[index].borrow_mut();
        for &byte in bytes {
            if byte == b'\n' {
                print(fd, &String::from_utf8_lossy(&line));
                line.clear();
            } else {
                line.push(byte);
            }
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __horizon_web_fetch(path: *const u8, len: usize, out: *mut u8) -> bool {
    let path = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(path, len) });
    let Some(bytes) = fetch_sync(&path) else {
        return false;
    };
    // std and this crate share one build of `Vec`.
    let out = unsafe { &mut *out.cast::<Vec<u8>>() };
    *out = bytes.to_vec();
    true
}

/// Threads waiting for a worker: `__horizon_std_thread_main` packets.
static QUEUE: Mutex<VecDeque<usize>> = Mutex::new(VecDeque::new());
static QUEUED: Condvar = Condvar::new();
/// Workers waiting for a thread. The page starts a larger pool for the game's first burst of
/// threads (web/www/main.js); after that, a taken worker is replaced only to keep this many
/// ready, or while threads wait for one: each worker holds a stack of its own.
static IDLE: AtomicUsize = AtomicUsize::new(0);
const RESERVE: usize = 8;

#[unsafe(no_mangle)]
pub extern "C" fn __horizon_web_spawn(packet: *mut u8) -> i32 {
    QUEUE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push_back(packet as usize);
    QUEUED.notify_one();
    0
}

/// A pooled worker: waits for a thread to run, has the page start a replacement when the reserve
/// runs low, runs the thread. The page then frees the worker (web/www/worker.js).
#[wasm_bindgen]
pub fn horizon_worker_main() {
    let (packet, replace) = {
        let mut queue = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
        IDLE.fetch_add(1, Ordering::Relaxed);
        loop {
            if let Some(packet) = queue.pop_front() {
                let idle = IDLE.fetch_sub(1, Ordering::Relaxed) - 1;
                break (packet, idle < RESERVE || !queue.is_empty());
            }
            queue = QUEUED.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
    };
    if replace {
        worker_taken();
    }
    unsafe { __horizon_std_thread_main(packet as *mut u8) };
}

/// The byte the page sets in the shared memory when a thread failed (web/www/worker.js): a
/// thread spinning on it stops instead of waiting forever.
#[wasm_bindgen]
pub fn horizon_web_failure_flag() -> usize {
    unsafe { __horizon_std_failure_flag() as usize }
}

/// The page's main thread calls this first: from now on it spins where std would block.
#[wasm_bindgen]
pub fn horizon_web_main_thread() {
    unsafe { __horizon_std_set_can_block(false) };
    // `std::env::current_exe` names this file (web/std): it exists, as a native executable
    // does, so what keys a cache to the running build (the resolved block behaviours'
    // snapshot) has a length and a modification time to read.
    let exe = std::env::current_exe().expect("web/std names the program");
    if let Some(dir) = exe.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&exe, env!("CARGO_PKG_VERSION"));
}

/// Registers the files under `fs/` that the game reads on demand: one `size<TAB>path` per line.
#[wasm_bindgen]
pub fn horizon_web_files(manifest: &str) {
    for line in manifest.lines() {
        let Some((size, path)) = line.split_once('\t') else {
            continue;
        };
        let Ok(size) = size.parse::<u64>() else {
            continue;
        };
        unsafe { __horizon_std_memfs_lazy(path.as_ptr(), path.len(), size) };
    }
}

/// Room for one file of a bundle (web/bundle.py): the page copies the bytes there as they arrive,
/// then hands them to the file system with `horizon_web_file_commit`.
#[wasm_bindgen]
pub fn horizon_web_file_alloc(size: usize) -> *mut u8 {
    let mut data = std::mem::ManuallyDrop::new(vec![0u8; size]);
    data.as_mut_ptr()
}

/// Adds the file `horizon_web_file_alloc` made room for.
#[wasm_bindgen]
pub fn horizon_web_file_commit(path: &str, data: *mut u8, size: usize) {
    // The Vec `horizon_web_file_alloc` leaked: `vec![0; size]` has exactly `size` capacity.
    let mut data = unsafe { Vec::from_raw_parts(data, size, size) };
    unsafe { __horizon_std_memfs_insert(path.as_ptr(), path.len(), (&raw mut data).cast()) };
}

/// Appends a command line argument for the game's `std::env::args`.
#[wasm_bindgen]
pub fn horizon_web_arg(arg: &str) {
    unsafe { __horizon_std_push_arg(arg.as_ptr(), arg.len()) };
}

#[wasm_bindgen]
pub fn horizon_web_env(key: &str, value: &str) {
    // Called before the game starts any thread.
    unsafe { std::env::set_var(key, value) };
}

/// Makes a folder (and its parents) and the working directory.
#[wasm_bindgen]
pub fn horizon_web_chdir(path: &str) -> Result<(), JsError> {
    std::fs::create_dir_all(path)?;
    std::env::set_current_dir(path)?;
    Ok(())
}
