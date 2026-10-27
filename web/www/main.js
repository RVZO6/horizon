// Starts the game: the wasm module with its worker pool, the file system, then horizon_run.
// `?arg=--world&arg=Test` passes command line arguments (docs/native-client.md).
import { start } from './threads.js';

addEventListener('unhandledrejection', (event) => console.error('horizon: unhandled', event.reason, event.reason?.stack));
addEventListener('error', (event) => console.error('horizon: error', event.message, event.error?.stack));

const horizon = await start({ pool: 32, files: true });
const home = '/home/web/horizon';
horizon.horizon_web_env('HOME', '/home/web');
horizon.horizon_web_env('HORIZON_HOME', home);
horizon.horizon_web_env('HORIZON_NO_DISCORD', '1');
horizon.horizon_web_chdir(home);
const args = ['--resources', '/res', ...new URLSearchParams(location.search).getAll('arg')];
for (const arg of args) horizon.horizon_web_arg(arg);
await horizon.horizon_run();
