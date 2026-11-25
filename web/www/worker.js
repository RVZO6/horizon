// A pooled thread (see crates/horizon-web): joins the page's module and memory, then waits on
// shared memory for the next std::thread::spawn and runs it.
import init, { horizon_worker_main } from './horizon.js';

// A thread that traps, or a worker that can't start, leaves whatever joins it waiting forever:
// mark the failure in the shared memory (horizon_web_failure_flag), which stops the spinning
// page thread, and tell the page.
function failed(data, error) {
    Atomics.store(new Uint8Array(data.memory.buffer), data.failure, 1);
    self.postMessage({ horizon: 'failed', error: String(error?.stack ?? error) });
}

self.onmessage = async ({ data }) => {
    // Where `worker_taken` (crates/horizon-web) asks the spawner for a replacement.
    self.horizonChannel = data.channel;
    let wasm;
    try {
        wasm = await init({ module_or_path: data.module, memory: data.memory, thread_stack_size: data.stack });
    } catch (error) {
        failed(data, error);
        throw error;
    }
    self.postMessage({ horizon: 'ready' });
    try {
        horizon_worker_main();
    } catch (error) {
        failed(data, error);
        throw error;
    }
    // The thread is done (std ran its exit): free this worker's stack and thread-local block,
    // which outlive the worker otherwise.
    wasm.__wbindgen_thread_destroy();
    self.close();
};
