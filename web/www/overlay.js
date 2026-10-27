// Vanilla's LoadingOverlay (the Mojang Studios screen) over the page while the game downloads and
// loads, then its fade into the game's first screen. The numbers are 26.3's
// (net.minecraft.client.gui.screens.LoadingOverlay): logo, bar, smoothing and the two fades.

const BACKGROUND = [239, 50, 61];
// The logo texture as the overlay addresses it, 120 by 120, whatever the PNG's size.
const LOGO_UV = 120;

/// Window.calculateScale(0, false): the largest scale that leaves at least 320 by 240.
function guiScale(width, height) {
    let scale = 1;
    while (width / (scale + 1) >= 320 && height / (scale + 1) >= 240) scale++;
    return scale;
}

export class LoadingOverlay {
    constructor(logoUrl) {
        this.canvas = document.createElement('canvas');
        this.canvas.className = 'horizon-overlay';
        document.body.append(this.canvas);
        this.context = this.canvas.getContext('2d');
        this.logo = new Image();
        this.logo.src = logoUrl;
        this.progress = 0;
        this.currentProgress = 0;
        this.fadeOutStart = -1;
        this.frame = this.frame.bind(this);
        requestAnimationFrame(this.frame);
    }

    /// ReloadInstance.getActualProgress: 0 to 1.
    setProgress(progress) {
        this.progress = Math.min(Math.max(progress, 0), 1);
    }

    /// The reload is done: the fades start (LoadingOverlay.tick).
    finish() {
        if (this.fadeOutStart < 0) this.fadeOutStart = performance.now();
    }

    frame(now) {
        const dpr = devicePixelRatio || 1;
        const width = Math.round(innerWidth * dpr);
        const height = Math.round(innerHeight * dpr);
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
        if (this.logo.complete && this.logo.naturalWidth > 0) {
            // RenderPipelines.MOJANG_LOGO blends SRC_ALPHA, ONE; the texture is blurred (linear).
            ctx.globalCompositeOperation = 'lighter';
            ctx.globalAlpha = logoAlpha;
            ctx.imageSmoothingEnabled = true;
            ctx.imageSmoothingQuality = 'high';
            const k = this.logo.naturalWidth / LOGO_UV;
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
            this.progressBar(guiWidth / 2 - halfWidth, barY - 5, guiWidth / 2 + halfWidth, barY + 5, 1 - Math.min(Math.max(fadeOut, 0), 1));
        }
        if (fadeOut >= 2) {
            this.canvas.remove();
            return;
        }
        requestAnimationFrame(this.frame);
    }

    progressBar(x0, y0, x1, y1, alpha) {
        x0 = Math.trunc(x0);
        x1 = Math.trunc(x1);
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
