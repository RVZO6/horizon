// Starts the game: vanilla's Mojang Studios screen over the downloads, the wasm module with its
// worker pool and file system, then horizon_run. `?arg=--world&arg=Test` passes command line
// arguments (docs/native-client.md).
import { LoadingOverlay } from './overlay.js';
import { start } from './threads.js';
import { loadBundle } from './files.js';

addEventListener('unhandledrejection', (event) => console.error('horizon: unhandled', event.reason, event.reason?.stack));
addEventListener('error', (event) => console.error('horizon: error', event.message, event.error?.stack));

// A browser starts sound only after a gesture: resume the game's audio (cpal's AudioContext) on
// the first click or key.
const audioContexts = [];
globalThis.AudioContext = class extends AudioContext {
    constructor(...args) {
        super(...args);
        audioContexts.push(this);
    }
};
for (const type of ['pointerdown', 'keydown']) {
    addEventListener(type, () => audioContexts.forEach((context) => context.resume()), true);
}

const t0 = performance.now();
const ms = () => Math.round(performance.now() - t0);
const overlay = new LoadingOverlay('fs/res/assets/minecraft/textures/gui/title/mojangstudios.png');

// The downloads start with the page. The bar follows their bytes, the thread workers' start, then
// the game's own start, and is full when the game has a screen to show.
const sizes = fetch('sizes.json').then((r) => r.json());
const received = { wasm: 0, bundle: 0, pool: 0, game: 0 };
const advance = async () => {
    const { wasm, startup } = await sizes;
    const downloads = Math.min((received.wasm + received.bundle) / (wasm + startup), 1);
    overlay.setProgress(0.8 * downloads + 0.1 * received.pool + 0.1 * received.game);
};
const counted = (response, onBytes) =>
    response.then((r) => {
        if (!r.ok) return r;
        const reader = r.body.getReader();
        const body = new ReadableStream({
            async pull(controller) {
                const { value, done } = await reader.read();
                if (done) return controller.close();
                onBytes(value.length);
                controller.enqueue(value);
            },
        });
        return new Response(body, { headers: { 'Content-Type': 'application/wasm' } });
    });
const wasm = counted(fetch('horizon_bg.wasm'), (n) => {
    received.wasm += n;
    advance();
});
const files = { index: fetch('fs/index.txt'), bundles: [fetch('fs/startup.bundle')] };
const onProgress = (done) => {
    received.bundle = done;
    advance();
};
addEventListener('horizon-loading', ({ detail }) => {
    received.game = detail.ready ? 1 : detail.progress;
    advance();
    if (detail.ready && !overlay.finished) {
        console.log(`horizon-web: title screen ready after ${ms()} ms`);
        overlay.finish();
    }
});

const onPool = (fraction) => {
    received.pool = fraction;
    advance();
};
const { horizon, memory } = await start({ pool: 32, wasm, files, onProgress, onPool });
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
// Pointer lock needs a click, as vanilla grabs the mouse on one; raw movement where offered.
addEventListener(
    'mousedown',
    () => {
        const canvas = document.querySelector('canvas:not(.horizon-overlay)');
        if (!canvas || document.pointerLockElement === canvas || !horizon.horizon_web_wants_pointer_lock()) return;
        const raw = canvas.requestPointerLock({ unadjustedMovement: true });
        raw?.catch?.(() => canvas.requestPointerLock());
    },
    true,
);
document.addEventListener('pointerlockchange', () => {
    const canvas = document.querySelector('canvas:not(.horizon-overlay)');
    horizon.horizon_web_pointer_lock_changed(!!canvas && document.pointerLockElement === canvas);
});
// Structure templates: until they arrive, the game fetches the ones it needs one by one.
loadBundle(horizon, memory, await fetch('fs/later.bundle')).then(
    (count) => console.log(`horizon-web: ${count} later files ready after ${ms()} ms`),
    (error) => console.error('horizon-web: later files', error),
);
