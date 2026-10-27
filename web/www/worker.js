// A pooled thread (see crates/horizon-web): joins the page's module and memory, then waits on
// shared memory for the next std::thread::spawn and runs it.
import init, { horizon_worker_main } from './horizon.js';

self.onmessage = async ({ data }) => {
    // Where `worker_taken` (crates/horizon-web) asks the spawner for a replacement.
    self.horizonChannel = data.channel;
    const wasm = await init({ module_or_path: data.module, memory: data.memory, thread_stack_size: data.stack });
    self.postMessage({ horizon: 'ready' });
    horizon_worker_main();
    // The thread is done (std ran its exit): free this worker's stack and thread-local block,
    // which outlive the worker otherwise.
    wasm.__wbindgen_thread_destroy();
    self.close();
};
