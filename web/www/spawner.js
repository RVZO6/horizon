// Keeps the pool of thread workers full (see crates/horizon-web). This worker never runs wasm and
// never blocks, so the workers it starts can start at once: a worker only starts when the thread
// that made it returns to its event loop, which the game's own threads never do.
self.onmessage = async ({ data: { module, memory, stack, channel, pool, failure } }) => {
    let started = 0;
    // A thread failed: the page shows it; the failure byte stops what waits on the thread.
    const failed = (error) => {
        Atomics.store(new Uint8Array(memory.buffer), failure, 1);
        self.postMessage({ horizon: 'thread-failed', error: String(error?.stack ?? error) });
    };
    const spawn = () =>
        new Promise((resolve, reject) => {
            const worker = new Worker(new URL('./worker.js', import.meta.url), {
                type: 'module',
                name: `horizon-${started++}`,
            });
            let ready = false;
            worker.onerror = (event) => {
                const error = event.error ?? new Error(event.message);
                if (ready) failed(error);
                else reject(error);
            };
            worker.onmessage = ({ data }) => {
                if (data?.horizon === 'ready') {
                    ready = true;
                    resolve(worker);
                }
                if (data?.horizon === 'failed') failed(data.error);
            };
            worker.postMessage({ module, memory, stack, channel, failure });
        });
    // A worker that takes a thread asks for a replacement here when the reserve runs low.
    new BroadcastChannel(channel).onmessage = () => {
        spawn().catch(failed);
    };
    try {
        // Each worker of the first pool, for the loading bar.
        const first = () => spawn().then(() => self.postMessage({ horizon: 'started' }));
        await Promise.all(Array.from({ length: pool }, first));
        self.postMessage({ horizon: 'ready' });
    } catch (error) {
        self.postMessage({ horizon: 'failed', error: String(error) });
    }
};
