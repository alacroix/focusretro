const SIZE = 24;

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

// Preload the base Dofus icon once for the lifetime of the app
let baseIconPromise: Promise<HTMLImageElement> | null = null;
function getBaseIcon(): Promise<HTMLImageElement> {
  if (!baseIconPromise) baseIconPromise = loadImage("/taskbar/dofus-icon.png");
  return baseIconPromise;
}

// Preload the skipped indicator once for the lifetime of the app
let zIconPromise: Promise<HTMLImageElement> | null = null;
function getZIcon(): Promise<HTMLImageElement> {
  if (!zIconPromise) zIconPromise = loadImage("/taskbar/z.png");
  return zIconPromise;
}

/**
 * Renders a 24×24 taskbar icon.
 *
 * Classic mode:
 *   1. Filled anti-aliased disc in `color` (if set)
 *   2. Dofus base icon at full size
 *   3. Class overlay icon at 16×16, bottom-right (if set), with 1px shadow
 *
 * Portrait mode:
 *   1. Colored ring border drawn first (behind)
 *   2. Class overlay icon clipped to circle (if set), otherwise colored disc
 *
 * Both modes: two z.png overlays (big + medium) at top-left when `isSkipped` is true.
 *
 * Returns the raw RGBA pixel array (24 * 24 * 4 = 2304 values).
 */
export async function renderAccountIcon(
  iconPath: string | null,
  color: string | null,
  iconStyle: "classic" | "portrait",
  isSkipped: boolean,
): Promise<number[]> {
  const canvas = document.createElement("canvas");
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext("2d")!;

  const colorValue = color ? (color.startsWith("#") ? color : `#${color}`) : null;

  if (iconStyle === "portrait") {
    // Portrait mode: colored ring first (behind), then portrait clipped inside

    // Ring border drawn first so portrait renders on top
    if (colorValue) {
      ctx.strokeStyle = colorValue;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 1, 0, Math.PI * 2);
      ctx.stroke();
    }

    if (iconPath) {
      try {
        const portrait = await loadImage(`/icons/${iconPath}.png`);
        ctx.save();
        ctx.beginPath();
        ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 2, 0, Math.PI * 2);
        ctx.clip();
        ctx.drawImage(portrait, 2, 2, SIZE - 4, SIZE - 4);
        ctx.restore();
      } catch {
        // portrait load failed — fall back to disc or base icon
        if (colorValue) {
          ctx.fillStyle = colorValue;
          ctx.beginPath();
          ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 2, 0, Math.PI * 2);
          ctx.fill();
        } else {
          const base = await getBaseIcon();
          ctx.drawImage(base, 0, 0, SIZE, SIZE);
        }
      }
    } else if (colorValue) {
      ctx.fillStyle = colorValue;
      ctx.beginPath();
      ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 2, 0, Math.PI * 2);
      ctx.fill();
    } else {
      // No icon and no color — fall back to Dofus base icon to avoid black box
      const base = await getBaseIcon();
      ctx.drawImage(base, 0, 0, SIZE, SIZE);
    }
  } else {
    // Classic mode

    // Layer 1: filled disc
    if (colorValue) {
      ctx.fillStyle = colorValue;
      ctx.beginPath();
      ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 1, 0, Math.PI * 2);
      ctx.fill();
    }

    // Layer 2: base Dofus icon
    const base = await getBaseIcon();
    ctx.drawImage(base, 0, 0, SIZE, SIZE);

    // Layer 3: class overlay with drop shadow
    if (iconPath) {
      try {
        const overlay = await loadImage(`/icons/${iconPath}.png`);
        // Shadow: semi-transparent offset copy
        ctx.globalAlpha = 0.4;
        ctx.drawImage(overlay, 9, 9, 16, 16);
        ctx.globalAlpha = 1.0;
        ctx.drawImage(overlay, 8, 8, 16, 16);
      } catch {
        // overlay load failed — skip silently
      }
    }
  }

  // Skipped indicator: two z.png overlays cascading top-left
  if (isSkipped) {
    try {
      const zImg = await getZIcon();
      ctx.drawImage(zImg, 0, 0, 10, 10); // big on top
      ctx.save();
      ctx.translate(10, 9);
      ctx.rotate(0.11); // ~6°
      ctx.drawImage(zImg, -3, -4, 8, 8); // medium tilted
      ctx.restore();
    } catch {
      // z.png load failed — skip silently
    }
  }

  return Array.from(ctx.getImageData(0, 0, SIZE, SIZE).data);
}
