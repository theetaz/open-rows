// SPDX-License-Identifier: Apache-2.0
import { contentHeight, logicalOffset } from './scroll';
import type { GridProps } from './model';
import { CanvasPainter } from './painter';
/** Coalesce native scroll/data events; only tile-window transitions allocate requests. */
export class CanvasController {
  private active = true;
  private frame = 0;
  private previous = -1;
  private preparation = 0;
  private dprQuery: MediaQueryList | undefined;
  private readonly last = { r0: -1, r1: -1, c0: -1, c1: -1, direction: 0 };
  private readonly observer: ResizeObserver;
  private readonly unsubscribe: () => void;
  constructor(
    private readonly scroll: HTMLElement,
    private readonly painter: CanvasPainter,
    private readonly props: GridProps,
  ) {
    this.observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      painter.resize(entry.contentRect.width, entry.contentRect.height);
      // Height changes alter the physical-to-logical mapping for capped spacers.
      this.update();
      this.prepare();
    });
    this.observer.observe(scroll);
    painter.resize(scroll.clientWidth, scroll.clientHeight);
    scroll.addEventListener('scroll', this.update, { passive: true });
    this.unsubscribe = props.source.subscribe(this.prepare);
    this.watchDpr();
    this.update();
    this.prepare();
  }
  /** Invalidate without creating a callback on every frame. */
  readonly invalidate = (): void => {
    if (this.active && !this.frame)
      this.frame = requestAnimationFrame(this.paint);
  };
  /** Track CSS-pixel offsets and request a new tile window only when it changes. */
  readonly update = (): void => {
    const started = performance.now();
    // Native scroll and geometry events read offsets; scripted writes wait for delivery.
    const y = logicalOffset(
        this.scroll.scrollTop,
        this.scroll.clientHeight,
        contentHeight(this.props),
      ),
      x = this.scroll.scrollLeft;
    const direction = Math.sign(y - this.previous);
    this.previous = y;
    this.painter.view.x = x;
    this.painter.view.y = y;
    this.painter.layout.visible(
      this.painter.range,
      this.painter.view,
      this.props.source.rows,
    );
    this.requestWindow(direction);
    this.invalidate();
    this.painter.metrics.scrollHandlerMs += performance.now() - started;
  };
  /** The row count changed: request the visible window again, even unmoved. */
  refetch(): void {
    this.last.r0 = -1;
    this.update();
  }
  /** Stop future callbacks before pending tile fills can settle. */
  destroy(): void {
    this.active = false;
    this.observer.disconnect();
    clearTimeout(this.preparation);
    this.dprQuery?.removeEventListener('change', this.dprChanged);
    this.unsubscribe();
    cancelAnimationFrame(this.frame);
    this.scroll.removeEventListener('scroll', this.update);
  }
  private readonly prepare = (): void => {
    if (!this.active || this.preparation) return;
    this.preparation = window.setTimeout(() => {
      this.preparation = 0;
      if (!this.active) return;
      this.painter.prepare();
      this.invalidate();
    }, 0);
  };
  private watchDpr(): void {
    this.dprQuery = window.matchMedia(
      `(resolution: ${String(window.devicePixelRatio || 1)}dppx)`,
    );
    this.dprQuery.addEventListener('change', this.dprChanged, { once: true });
  }
  private readonly dprChanged = (): void => {
    this.painter.resize(this.painter.view.width, this.painter.view.height);
    this.watchDpr();
    this.prepare();
  };
  private readonly paint = (): void => {
    this.frame = 0;
    this.painter.paint();
  };
  private requestWindow(direction: number): void {
    const { r0, r1, c0, c1 } = this.painter.range;
    const start = r0 >> 8,
      end = r1 >> 8;
    if (
      start === this.last.r0 &&
      end === this.last.r1 &&
      Math.floor(c0 / 20) === this.last.c0 &&
      Math.floor(c1 / 20) === this.last.c1 &&
      direction === this.last.direction
    )
      return;
    this.last.r0 = start;
    this.last.r1 = end;
    this.last.c0 = Math.floor(c0 / 20);
    this.last.c1 = Math.floor(c1 / 20);
    this.last.direction = direction;
    void this.props.source
      .window(this.painter.range, direction)
      .catch((error: unknown) => {
        if (this.active) this.props.onError(error);
      });
  }
}
