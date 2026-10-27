// Streams a bundle (web/bundle.py) into the game's file system: each file's bytes are copied into
// wasm memory as they arrive, then handed over with horizon_web_file_commit.

export async function loadBundle(horizon, memory, response, onProgress = () => {}) {
    if (!response.ok) throw new Error(`${response.url}: HTTP ${response.status}`);
    const reader = response.body.getReader();
    let pending = new Uint8Array(0);
    let entries = null;
    let total = 0;
    let done = 0;
    let current = null; // { path, size, ptr, filled }
    let next = 0;
    const startFile = () => {
        while (next < entries.length) {
            const { path, size } = entries[next++];
            const ptr = horizon.horizon_web_file_alloc(size);
            if (size > 0) {
                current = { path, size, ptr, filled: 0 };
                return;
            }
            horizon.horizon_web_file_commit(path, ptr, 0);
        }
        current = null;
    };
    const take = (chunk) => {
        let offset = 0;
        if (entries === null) {
            const joined = new Uint8Array(pending.length + chunk.length);
            joined.set(pending);
            joined.set(chunk, pending.length);
            if (joined.length < 8) {
                pending = joined;
                return;
            }
            const view = new DataView(joined.buffer);
            if (view.getUint32(0, false) !== 0x485a4231) throw new Error(`${response.url}: not a bundle`);
            const length = view.getUint32(4, true);
            if (joined.length < 8 + length) {
                pending = joined;
                return;
            }
            const index = new TextDecoder().decode(joined.subarray(8, 8 + length));
            entries = index
                .split('\n')
                .filter((line) => line)
                .map((line) => {
                    const tab = line.indexOf('\t');
                    return { size: Number(line.slice(0, tab)), path: line.slice(tab + 1) };
                });
            total = entries.reduce((sum, e) => sum + e.size, 0);
            pending = null;
            chunk = joined;
            offset = 8 + length;
            startFile();
        }
        while (offset < chunk.length && current !== null) {
            const count = Math.min(current.size - current.filled, chunk.length - offset);
            // The memory may have grown since the last view.
            new Uint8Array(memory.buffer, current.ptr + current.filled, count).set(
                chunk.subarray(offset, offset + count),
            );
            current.filled += count;
            offset += count;
            done += count;
            if (current.filled === current.size) {
                horizon.horizon_web_file_commit(current.path, current.ptr, current.size);
                startFile();
            }
        }
    };
    for (;;) {
        const { value, done: finished } = await reader.read();
        if (finished) break;
        take(value);
        onProgress(done, total);
    }
    if (entries === null || current !== null || next < entries.length) {
        throw new Error(`${response.url}: the bundle ended early`);
    }
    return entries.length;
}
