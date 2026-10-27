// Keeps the pool of thread workers full (see crates/horizon-web). This worker never runs wasm and
// never blocks, so the workers it starts can start at once: a worker only starts when the thread
// that made it returns to its event loop, which the game's own threads never do.
self.onmessage = async ({ data: { module, memory, stack, channel, pool } }) => {
    let started = 0;
    const spawn = () =>
        new Promise((resolve, reject) => {
            const worker = new Worker(new URL('./worker.js', import.meta.url), {
                type: 'module',
                name: `horizon-${started++}`,
            });
            worker.onerror = (event) => reject(event.error ?? new Error(event.message));
            worker.onmessage = ({ data }) => {
                if (data?.horizon === 'ready') resolve(worker);
            };
            worker.postMessage({ module, memory, stack, channel });
        });
    // A worker that takes a thread asks for its replacement here.
    new BroadcastChannel(channel).onmessage = () => {
        spawn().catch((error) => console.error('horizon: a thread worker failed to start', error));
    };
    try {
        await Promise.all(Array.from({ length: pool }, spawn));
        self.postMessage({ horizon: 'ready' });
    } catch (error) {
        self.postMessage({ horizon: 'failed', error: String(error) });
    }
};
