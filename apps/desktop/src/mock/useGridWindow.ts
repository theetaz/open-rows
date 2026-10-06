// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useRef, useState } from 'react';
import { geometry } from '@openrows/ui/tokens';
const rowHeight = parseFloat(geometry['grid-row-height']);
const overscan = 3;
/** Keep the DOM bounded by the viewport, including after a panel resize. */
export function useGridWindow(count: number): {
  ref: React.RefObject<HTMLDivElement | null>;
  start: number;
  end: number;
  top: number;
  bottom: number;
  onScroll: () => void;
  reveal: (index: number) => void;
} {
  const ref = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState({ top: 0, height: 520 });
  const measure = (): void => {
    const grid = ref.current;
    if (grid)
      setViewport({ top: grid.scrollTop, height: grid.clientHeight || 520 });
  };
  useLayoutEffect(() => {
    const grid = ref.current;
    if (!grid) return;
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    return () => {
      observer.disconnect();
    };
  }, []);
  const start = Math.min(
    count,
    Math.max(0, Math.floor(viewport.top / rowHeight) - overscan),
  );
  const end = Math.min(
    count,
    Math.ceil((viewport.top + viewport.height) / rowHeight) + overscan,
  );
  const reveal = (index: number): void => {
    const grid = ref.current;
    if (!grid) return;
    const top = index * rowHeight;
    const height = grid.clientHeight || 520;
    if (top < grid.scrollTop) grid.scrollTop = top;
    else if (top + rowHeight * 2 > grid.scrollTop + height)
      grid.scrollTop = top + rowHeight * 2 - height;
    measure();
  };
  return {
    ref,
    start,
    end,
    top: start * rowHeight,
    bottom: (count - end) * rowHeight,
    onScroll: measure,
    reveal,
  };
}
