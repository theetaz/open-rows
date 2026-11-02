// SPDX-License-Identifier: Apache-2.0
import type { GridProps } from './model';

// WebKit clamps layout dimensions at 33,554,428 CSS pixels (2^25 minus 4).
const MAX_HEIGHT = 32_000_000;
// A remembered scripted offset still applies while the native offset is within
// one pixel of it, which covers WebKit's rounding of written offsets.
const SAME_NATIVE = 1;

/** Logical content height includes the fixed header row. */
function contentHeight(props: GridProps): number {
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

/** Wheel distance in CSS pixels for each `WheelEvent.deltaMode`. */
function wheelPixels(
  delta: number,
  mode: number,
  line: number,
  page: number,
): number {
  if (mode === WheelEvent.DOM_DELTA_LINE) return delta * line;
  return mode === WheelEvent.DOM_DELTA_PAGE ? delta * page : delta;
}

/**
 * Vertical position of a grid whose native spacer may be capped.
 *
 * Uncapped grids use native offsets unchanged. Capped grids map scrollbar
 * drags proportionally onto the logical rows, while wheel steps and scripted
 * reveals move by exact logical pixels: the logical target is remembered for
 * as long as the native offset still shows it.
 */
export class VerticalScroll {
  private target = { native: Number.NaN, logical: 0 };
  private capped = false;
  private moved: () => void = () => undefined;
  constructor(
    private readonly scroll: HTMLElement,
    private readonly spacer: HTMLElement,
    private readonly props: GridProps,
  ) {}
  /** Size the spacer and intercept wheel input only while the spacer is capped. */
  fit(): void {
    const content = contentHeight(this.props),
      capped = content > MAX_HEIGHT;
    this.spacer.style.height = `${String(spacerHeight(content))}px`;
    if (!capped) this.target.native = Number.NaN;
    if (capped === this.capped) return;
    this.capped = capped;
    if (capped)
      this.scroll.addEventListener('wheel', this.wheel, { passive: false });
    else this.scroll.removeEventListener('wheel', this.wheel);
  }
  /** Logical offset of the first content pixel below the top edge. */
  offset(): number {
    const native = this.scroll.scrollTop;
    if (!this.capped) return native;
    const viewport = this.scroll.clientHeight,
      content = contentHeight(this.props);
    if (Math.abs(native - this.target.native) < SAME_NATIVE)
      return Math.min(this.target.logical, Math.max(0, content - viewport));
    return logicalOffset(native, viewport, content);
  }
  /** Native offset that shows a logical offset; capped targets are remembered. */
  toNative(y: number): number {
    if (!this.capped) return y;
    const viewport = this.scroll.clientHeight,
      content = contentHeight(this.props),
      logical = Math.min(Math.max(0, content - viewport), Math.max(0, y)),
      native = nativeOffset(logical, viewport, content);
    this.target = { native, logical };
    return native;
  }
  /** Scroll to a logical offset. */
  scrollTo(y: number): void {
    this.scroll.scrollTop = this.toNative(y);
  }
  /**
   * Called after each intercepted wheel step. A step can move the native
   * offset by less than a pixel, which WebKit does not report as a scroll.
   */
  onWheel(moved: () => void): void {
    this.moved = moved;
  }
  /** Stop intercepting wheel input. */
  destroy(): void {
    this.scroll.removeEventListener('wheel', this.wheel);
  }
  private readonly wheel = (event: WheelEvent): void => {
    // Ctrl+wheel is pinch zoom on macOS and is left to the webview.
    if (event.ctrlKey) return;
    const { rowHeight } = this.props.theme,
      page = this.scroll.clientHeight,
      dy = wheelPixels(event.deltaY, event.deltaMode, rowHeight, page),
      dx = wheelPixels(event.deltaX, event.deltaMode, rowHeight, page);
    event.preventDefault();
    if (dy) this.scrollTo(this.offset() + dy);
    if (dx) this.scroll.scrollLeft += dx;
    this.moved();
  };
}
