// Check before downloading the game. Some hardened browsers deliberately disable these APIs.
export function requireBrowserFeatures(scope = globalThis) {
    if (!scope.isSecureContext) {
        throw new Error('Horizon needs a secure HTTPS connection. Open the HTTPS game address.');
    }
    if (!scope.navigator?.gpu && typeof scope.WebGL2RenderingContext !== 'function') {
        throw new Error('Neither WebGPU nor WebGL2 is available in this browser. Horizon needs one of these graphics APIs.');
    }
    if (!scope.crossOriginIsolated || typeof scope.SharedArrayBuffer !== 'function') {
        throw new Error('Shared memory is unavailable. Horizon needs WebAssembly threads and cross-origin isolation.');
    }
    if (typeof scope.WebAssembly?.compileStreaming !== 'function') {
        throw new Error('WebAssembly is unavailable in this browser. Horizon requires WebAssembly.');
    }
}

export function showFailure(error) {
    console.error('horizon: stopped', error);
    if (document.querySelector('.horizon-stopped')) return;
    const box = document.createElement('div');
    box.className = 'horizon-stopped';
    const reason = String(error?.message ?? error).slice(0, 500);
    box.textContent = `Horizon stopped. ${reason}`;
    document.body.append(box);
}
