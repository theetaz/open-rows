// SPDX-License-Identifier: Apache-2.0
import type { Point } from './layout';

/** Pan offset (screen pixels) and zoom of the diagram content. */
export type View = { x: number; y: number; scale: number };
export type Size = { width: number; height: number };

const MIN_SCALE = 0.05;
const MAX_SCALE = 3;
const FIT_MARGIN = 16;

const clampScale = (scale: number): number =>
  Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));

/** The whole diagram centred in the viewport, never enlarged beyond 100 %. */
export function fitView(content: Size, viewport: Size): View {
  const room = {
    width: Math.max(viewport.width - 2 * FIT_MARGIN, 1),
    height: Math.max(viewport.height - 2 * FIT_MARGIN, 1),
  };
  const scale = clampScale(
    Math.min(
      room.width / Math.max(content.width, 1),
      room.height / Math.max(content.height, 1),
      1,
    ),
  );
  return {
    scale,
    x: (viewport.width - content.width * scale) / 2,
    y: (viewport.height - content.height * scale) / 2,
  };
}

/** Zoom by `factor`, keeping the diagram point under `at` (screen) still. */
export function zoomAt(view: View, factor: number, at: Point): View {
  const scale = clampScale(view.scale * factor);
  const ratio = scale / view.scale;
  return {
    scale,
    x: at.x - (at.x - view.x) * ratio,
    y: at.y - (at.y - view.y) * ratio,
  };
}

/** Pan so the diagram point `target` sits in the middle of the viewport. */
export function centreOn(view: View, target: Point, viewport: Size): View {
  return {
    scale: view.scale,
    x: viewport.width / 2 - target.x * view.scale,
    y: viewport.height / 2 - target.y * view.scale,
  };
}
