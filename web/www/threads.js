// Starts the wasm module on the page's thread with a pool of workers ready to run its threads.
import init, * as horizon from './horizon.js';
import { loadBundle } from './files.js';

// Every thread's stack, as the main thread's (web/build.sh): wasm has no guard page.
const STACK = 8 * 1024 * 1024;

// `files`: the responses (or promises of them) of fs/index.txt and the bundles to load before the
// game starts (web/bundle.py). `onProgress(done, total)` follows the bundles' bytes.
export async function start({ pool = 32, files = null, onProgress = () => {} } = {}) {
    const exports = await init();
    const module = init.__wbindgen_wasm_module;
    const memory = exports.memory;
    horizon.horizon_web_main_thread();
    if (files !== null) {
        const index = await files.index;
        if (!index.ok) throw new Error(`${index.url}: HTTP ${index.status}`);
        horizon.horizon_web_files(await index.text());
        for (const bundle of files.bundles ?? []) {
            await loadBundle(horizon, memory, await bundle, onProgress);
        }
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
    return { horizon, memory };
}
