// SPDX-License-Identifier: Apache-2.0
import type { GridProps } from './model';

// WebKit clamps layout dimensions near 2^25 CSS pixels.
const MAX_HEIGHT = 32_000_000;

/** Logical content height includes the fixed header row. */
export function contentHeight(props: GridProps): number {
  return (props.source.rows + 1) * props.theme.rowHeight;
}

/** Keep the native spacer inside WebKit's representable layout range. */
export function spacerHeight(content: number): number {
  return Math.min(content, MAX_HEIGHT);
}

/** Convert native scrolling to the full logical range, preserving the last row. */
export function logicalOffset(
  offset: number,
  viewport: number,
  content: number,
): number {
  const physical = Math.max(0, spacerHeight(content) - viewport);
  const logical = Math.max(0, content - viewport);
  return physical > 0
    ? (Math.min(physical, Math.max(0, offset)) / physical) * logical
    : 0;
}

/** Convert reveals and restored logical offsets to native scroll coordinates. */
export function nativeOffset(
  offset: number,
  viewport: number,
  content: number,
): number {
  const physical = Math.max(0, spacerHeight(content) - viewport);
  const logical = Math.max(0, content - viewport);
  return logical > 0
    ? (Math.min(logical, Math.max(0, offset)) / logical) * physical
    : 0;
}
