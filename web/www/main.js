// Starts the game: the wasm module with its worker pool, the file system, then horizon_run.
// `?arg=--world&arg=Test` passes command line arguments (docs/native-client.md).
import { start } from './threads.js';
import { loadBundle } from './files.js';

addEventListener('unhandledrejection', (event) => console.error('horizon: unhandled', event.reason, event.reason?.stack));
addEventListener('error', (event) => console.error('horizon: error', event.message, event.error?.stack));

const t0 = performance.now();
const ms = () => Math.round(performance.now() - t0);
// The downloads start with the page, beside the wasm module's.
const files = { index: fetch('fs/index.txt'), bundles: [fetch('fs/startup.bundle')] };
const { horizon, memory } = await start({ pool: 32, files });
// For the console and tests: the wasm memory's size.
globalThis.horizonMemory = memory;
console.log(`horizon-web: wasm and startup files ready after ${ms()} ms`);
const home = '/home/web/horizon';
horizon.horizon_web_env('HOME', '/home/web');
horizon.horizon_web_env('HORIZON_HOME', home);
horizon.horizon_web_env('HORIZON_NO_DISCORD', '1');
horizon.horizon_web_chdir(home);
const args = ['--resources', '/res', ...new URLSearchParams(location.search).getAll('arg')];
for (const arg of args) horizon.horizon_web_arg(arg);
await horizon.horizon_run();
// Structure templates: until they arrive, the game fetches the ones it needs one by one.
loadBundle(horizon, memory, await fetch('fs/later.bundle')).then(
    (count) => console.log(`horizon-web: ${count} later files ready after ${ms()} ms`),
    (error) => console.error('horizon-web: later files', error),
);
