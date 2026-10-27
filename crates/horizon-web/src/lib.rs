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

use std::cell::RefCell;
use std::collections::VecDeque;
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
}

unsafe extern "C" {
    fn __horizon_std_set_can_block(can_block: bool);
    fn __horizon_std_thread_main(packet: *mut u8);
    fn __horizon_std_push_arg(ptr: *const u8, len: usize);
    fn __horizon_std_memfs_lazy(ptr: *const u8, len: usize, size: u64);
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

#[unsafe(no_mangle)]
pub extern "C" fn __horizon_web_spawn(packet: *mut u8) -> i32 {
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push_back(packet as usize);
    QUEUED.notify_one();
    0
}

/// A pooled worker: waits for a thread to run, tells the page to start a replacement, runs it.
#[wasm_bindgen]
pub fn horizon_worker_main() {
    let packet = {
        let mut queue = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(packet) = queue.pop_front() {
                break packet;
            }
            queue = QUEUED.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
    };
    worker_taken();
    unsafe { __horizon_std_thread_main(packet as *mut u8) };
}

/// The page's main thread calls this first: from now on it spins where std would block.
#[wasm_bindgen]
pub fn horizon_web_main_thread() {
    unsafe { __horizon_std_set_can_block(false) };
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
