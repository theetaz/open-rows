// SPDX-License-Identifier: Apache-2.0
import type { Size } from './viewport';

/** WebKit refuses larger canvases; stay well inside its limits. */
const MAX_PIXELS = 16_000_000;
const MAX_SIDE = 16_384;

/** Device pixels per diagram unit: 2× unless that exceeds the canvas limits. */
export function pngScale(size: Size): number {
  const area = Math.max(size.width * size.height, 1);
  return Math.min(
    2,
    Math.sqrt(MAX_PIXELS / area),
    MAX_SIDE / Math.max(size.width, size.height, 1),
  );
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => {
      resolve(image);
    };
    image.onerror = () => {
      reject(new Error('The diagram could not be drawn as an image'));
    };
    image.src = url;
  });
}

/** Draw standalone SVG text onto a canvas and encode it as PNG bytes. */
export async function svgToPng(svg: string, size: Size): Promise<Uint8Array> {
  const scale = pngScale(size);
  const url = URL.createObjectURL(
    new Blob([svg], { type: 'image/svg+xml;charset=utf-8' }),
  );
  try {
    const image = await loadImage(url);
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(size.width * scale));
    canvas.height = Math.max(1, Math.round(size.height * scale));
    const context = canvas.getContext('2d');
    if (!context) throw new Error('Canvas drawing is unavailable');
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const blob = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob(resolve, 'image/png');
    });
    if (!blob) throw new Error('The diagram is too large for a PNG');
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}
