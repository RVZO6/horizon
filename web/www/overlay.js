// Vanilla's Mojang Studios screen over the page while the game downloads and loads, then its fade
// into the game. It draws on a worker (an OffscreenCanvas) where the browser allows, so it keeps
// moving while the page's thread is busy; else on the page's thread (overlay-draw.js).
import { OverlayDrawing, animate, loadLogo } from './overlay-draw.js';

export class LoadingOverlay {
    constructor(logoUrl) {
        this.canvas = document.createElement('canvas');
        this.canvas.className = 'horizon-overlay';
        document.body.append(this.canvas);
        const size = () => [Math.round(innerWidth * (devicePixelRatio || 1)), Math.round(innerHeight * (devicePixelRatio || 1))];
        const done = () => {
            this.canvas.remove();
            this.worker?.terminate();
            // The game owns the window now: nothing red behind its canvas.
            document.documentElement.classList.add('horizon-running');
        };
        const logo = new URL(logoUrl, location.href).href;
        if (this.canvas.transferControlToOffscreen) {
            const offscreen = this.canvas.transferControlToOffscreen();
            this.worker = new Worker(new URL('./overlay-worker.js', import.meta.url), { type: 'module', name: 'horizon-overlay' });
            const [width, height] = size();
            this.worker.postMessage({ canvas: offscreen, width, height, logo }, [offscreen]);
            this.worker.onmessage = ({ data }) => data.done && done();
            this.send = (message) => this.worker.postMessage(message);
        } else {
            const drawing = new OverlayDrawing(this.canvas);
            drawing.resize(...size());
            loadLogo(logo).then((bitmap) => (drawing.logo = bitmap), (error) => console.error(error));
            animate(drawing, done);
            this.send = (message) => {
                if (message.resize) drawing.resize(...message.resize);
                if (message.progress !== undefined) drawing.setProgress(message.progress);
                if (message.finish) drawing.finish(performance.now());
            };
        }
        addEventListener('resize', () => this.send({ resize: size() }));
    }

    /// ReloadInstance.getActualProgress: 0 to 1.
    setProgress(progress) {
        if (progress !== this.sent) this.send({ progress: (this.sent = progress) });
    }

    /// The reload is done: the fades start.
    finish() {
        if (!this.finished) this.send({ finish: (this.finished = true) });
    }
}
