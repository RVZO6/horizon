import { test } from 'node:test';
import assert from 'node:assert/strict';
import { requireBrowserFeatures, showFailure } from './www/startup.js';

const supported = () => ({
    isSecureContext: true,
    navigator: { gpu: {} },
    crossOriginIsolated: true,
    SharedArrayBuffer: class {},
    WebAssembly: { compileStreaming() {} },
});

test('supported browsers can bootstrap', () => {
    assert.doesNotThrow(() => requireBrowserFeatures(supported()));
});

test('missing both graphics APIs reports the requirement before downloading', () => {
    const scope = supported();
    delete scope.navigator.gpu;
    assert.throws(() => requireBrowserFeatures(scope), /Neither WebGPU nor WebGL2/);
});

test('WebGL2-only browsers can bootstrap the fallback', () => {
    const scope = supported();
    delete scope.navigator.gpu;
    scope.WebGL2RenderingContext = class {};
    assert.doesNotThrow(() => requireBrowserFeatures(scope));
});

test('plain HTTP explains the secure-context requirement', () => {
    assert.throws(() => requireBrowserFeatures({ ...supported(), isSecureContext: false }), /HTTPS/);
});

test('missing shared memory or isolation explains the threading requirement', () => {
    assert.throws(() => requireBrowserFeatures({ ...supported(), SharedArrayBuffer: undefined }), /Shared memory/);
    assert.throws(() => requireBrowserFeatures({ ...supported(), crossOriginIsolated: false }), /Shared memory/);
});

test('disabled WebAssembly reports the requirement', () => {
    assert.throws(() => requireBrowserFeatures({ ...supported(), WebAssembly: undefined }), /WebAssembly is unavailable/);
});

test('failure details are visible as text and duplicate failures do not add more boxes', (t) => {
    t.mock.method(console, 'error', () => {});
    const original = globalThis.document;
    const boxes = [];
    globalThis.document = {
        querySelector: () => boxes[0],
        createElement: () => ({}),
        body: { append: (box) => boxes.push(box) },
    };
    try {
        showFailure(new Error('WebGPU <unavailable>'));
        showFailure(new Error('another error'));
        assert.equal(boxes.length, 1);
        assert.equal(boxes[0].className, 'horizon-stopped');
        assert.equal(boxes[0].textContent, 'Horizon stopped. WebGPU <unavailable>');
    } finally {
        if (original === undefined) delete globalThis.document;
        else globalThis.document = original;
    }
});
