// Starts the wasm module on the page's thread with a pool of workers ready to run its threads.
import init, * as horizon from './horizon.js';

// Every thread's stack, as the main thread's (web/build.sh): wasm has no guard page.
const STACK = 8 * 1024 * 1024;

export async function start({ pool = 32, files = null } = {}) {
    const exports = await init();
    const module = init.__wbindgen_wasm_module;
    const memory = exports.memory;
    horizon.horizon_web_main_thread();
    if (files !== null) {
        const response = await fetch('fs/index.txt');
        if (!response.ok) throw new Error(`fs/index.txt: HTTP ${response.status}`);
        horizon.horizon_web_files(await response.text());
    }
    const spawner = new Worker(new URL('./spawner.js', import.meta.url), { type: 'module', name: 'horizon-spawner' });
    await new Promise((resolve, reject) => {
        spawner.onerror = (event) => reject(event.error ?? new Error(event.message));
        spawner.onmessage = ({ data }) => {
            if (data?.horizon === 'ready') resolve();
            if (data?.horizon === 'failed') reject(new Error(data.error));
        };
        const channel = `horizon-threads-${crypto.randomUUID()}`;
        spawner.postMessage({ module, memory, stack: STACK, channel, pool });
    });
    return horizon;
}
