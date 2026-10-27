// A pooled thread (see crates/horizon-web): joins the page's module and memory, then waits on
// shared memory for the next std::thread::spawn and runs it.
import init, { horizon_worker_main } from './horizon.js';

self.onmessage = async ({ data }) => {
    // Where `worker_taken` (crates/horizon-web) asks the spawner for a replacement.
    self.horizonChannel = data.channel;
    await init({ module_or_path: data.module, memory: data.memory, thread_stack_size: data.stack });
    self.postMessage({ horizon: 'ready' });
    horizon_worker_main();
    self.close();
};
