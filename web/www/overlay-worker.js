// The Mojang Studios screen on a worker (overlay.js): it keeps drawing while the page's thread is
// busy starting the game.
import { OverlayDrawing, animate, loadLogo } from './overlay-draw.js';

let drawing = null;
self.onmessage = ({ data }) => {
    if (data.canvas) {
        drawing = new OverlayDrawing(data.canvas);
        drawing.resize(data.width, data.height);
        loadLogo(data.logo).then(
            (logo) => (drawing.logo = logo),
            (error) => console.error('horizon: the Mojang Studios logo', error),
        );
        animate(drawing, () => self.postMessage({ done: true }));
    }
    if (data.resize) drawing.resize(data.resize[0], data.resize[1]);
    if (data.progress !== undefined) drawing.setProgress(data.progress);
    if (data.finish) drawing.finish(performance.now());
};
