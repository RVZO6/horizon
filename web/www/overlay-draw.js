// Vanilla's LoadingOverlay (the Mojang Studios screen), drawn on a 2D canvas: 26.3's numbers
// (net.minecraft.client.gui.screens.LoadingOverlay) for the logo, the bar, its smoothing and the
// two fades. overlay.js runs it on a worker, so it keeps moving while the page's thread is busy.

const BACKGROUND = [239, 50, 61];
// The logo texture as the overlay addresses it, 120 by 120, whatever the PNG's size.
const LOGO_UV = 120;

/// Window.calculateScale(0, false): the largest scale that leaves at least 320 by 240.
function guiScale(width, height) {
    let scale = 1;
    while (width / (scale + 1) >= 320 && height / (scale + 1) >= 240) scale++;
    return scale;
}

export class OverlayDrawing {
    /// `canvas`: a canvas or an OffscreenCanvas; `logo`: an ImageBitmap, or null until it loads.
    constructor(canvas) {
        this.canvas = canvas;
        this.context = canvas.getContext('2d');
        this.logo = null;
        this.width = canvas.width;
        this.height = canvas.height;
        this.progress = 0;
        this.currentProgress = 0;
        this.fadeOutStart = -1;
    }

    resize(width, height) {
        this.width = width;
        this.height = height;
    }

    /// ReloadInstance.getActualProgress: 0 to 1.
    setProgress(progress) {
        this.progress = Math.min(Math.max(progress, 0), 1);
    }

    /// The reload is done: the fades start (LoadingOverlay.tick).
    finish(now) {
        if (this.fadeOutStart < 0) this.fadeOutStart = now;
    }

    /// One frame at `now` (ms); false once the overlay is gone.
    draw(now) {
        const { width, height } = this;
        if (this.canvas.width !== width || this.canvas.height !== height) {
            this.canvas.width = width;
            this.canvas.height = height;
        }
        const scale = guiScale(width, height);
        const guiWidth = Math.ceil(width / scale);
        const guiHeight = Math.ceil(height / scale);
        const fadeOut = this.fadeOutStart > -1 ? (now - this.fadeOutStart) / 1000 : -1;
        const ctx = this.context;
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.clearRect(0, 0, width, height);
        if (fadeOut >= 2) return false;
        let logoAlpha;
        if (fadeOut >= 1) {
            // The game's screen shows through.
            const alpha = Math.ceil((1 - Math.min(Math.max(fadeOut - 1, 0), 1)) * 255) / 255;
            ctx.fillStyle = `rgba(${BACKGROUND.join(',')},${alpha})`;
            ctx.fillRect(0, 0, width, height);
            logoAlpha = 1 - Math.min(Math.max(fadeOut - 1, 0), 1);
        } else {
            ctx.fillStyle = `rgb(${BACKGROUND.join(',')})`;
            ctx.fillRect(0, 0, width, height);
            logoAlpha = 1;
        }
        ctx.setTransform(scale, 0, 0, scale, 0, 0);
        const centerX = Math.trunc(guiWidth * 0.5);
        const centerY = Math.trunc(guiHeight * 0.5);
        const logoHeight = Math.min(guiWidth * 0.75, guiHeight) * 0.25;
        const halfHeight = Math.trunc(logoHeight * 0.5);
        const halfWidth = Math.trunc(logoHeight * 4 * 0.5);
        if (this.logo) {
            // RenderPipelines.MOJANG_LOGO blends SRC_ALPHA, ONE; the texture is blurred (linear).
            ctx.globalCompositeOperation = 'lighter';
            ctx.globalAlpha = logoAlpha;
            ctx.imageSmoothingEnabled = true;
            ctx.imageSmoothingQuality = 'high';
            const k = this.logo.width / LOGO_UV;
            const h = Math.trunc(logoHeight);
            ctx.drawImage(this.logo, -0.0625 * k, 0, 120 * k, 60 * k, centerX - halfWidth, centerY - halfHeight, halfWidth, h);
            ctx.drawImage(this.logo, 0.0625 * k, 60 * k, 120 * k, 60 * k, centerX, centerY - halfHeight, halfWidth, h);
            ctx.globalCompositeOperation = 'source-over';
            ctx.globalAlpha = 1;
        }
        // Smoothed once a frame, as vanilla does.
        this.currentProgress = Math.min(Math.max(this.currentProgress * 0.95 + this.progress * 0.050000012, 0), 1);
        if (fadeOut < 1) {
            const barY = Math.trunc(guiHeight * 0.8325);
            const half = Math.trunc(guiWidth / 2);
            this.progressBar(half - halfWidth, barY - 5, half + halfWidth, barY + 5, 1 - Math.min(Math.max(fadeOut, 0), 1));
        }
        return true;
    }

    progressBar(x0, y0, x1, y1, alpha) {
        const fill = Math.ceil((x1 - x0 - 2) * this.currentProgress);
        const ctx = this.context;
        ctx.fillStyle = `rgba(255,255,255,${Math.round(alpha * 255) / 255})`;
        // GuiGraphics.fill orders each pair of corners.
        const rect = (ax, ay, bx, by) =>
            ctx.fillRect(Math.min(ax, bx), Math.min(ay, by), Math.abs(bx - ax), Math.abs(by - ay));
        rect(x0 + 2, y0 + 2, x0 + fill, y1 - 2);
        rect(x0 + 1, y0, x1 - 1, y0 + 1);
        rect(x0 + 1, y1, x1 - 1, y1 - 1);
        rect(x0, y0, x0 + 1, y1);
        rect(x1, y0, x1 - 1, y1);
    }
}

/// Runs `drawing` a frame at a time until the overlay is gone, then calls `done`.
export function animate(drawing, done) {
    const frame = typeof requestAnimationFrame === 'function' ? requestAnimationFrame : (f) => setTimeout(() => f(performance.now()), 16);
    const step = (now) => (drawing.draw(now) ? frame(step) : done());
    frame(step);
}

export async function loadLogo(url) {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
    return createImageBitmap(await response.blob());
}
