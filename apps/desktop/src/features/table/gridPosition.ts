// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import type { GridHandle, SelectionModel } from '@openrows/grid';

/** Where a table grid was scrolled and focused when its page was replaced. */
type GridPosition = { x: number; y: number; row: number; column: number };

/**
 * A tab's memory of its grid position across page loads; `restore` asks
 * the next page (a reload of the same view) to return to it.
 */
export type PositionMemory = {
  position: GridPosition | null;
  restore: boolean;
};

/** Ask the next mounted page of the tab to return to the remembered position. */
export function restoreNext(memory: React.RefObject<PositionMemory>): void {
  memory.current.restore = memory.current.position !== null;
}

/** The position to return to, once: the request is consumed. */
function takeRestore(
  memory: React.RefObject<PositionMemory>,
): GridPosition | null {
  const { position, restore } = memory.current;
  memory.current.restore = false;
  return restore ? position : null;
}

/**
 * Remember the grid's scroll and focused cell when the page unmounts, and
 * return to them on the next mount after a reload (FR-DATA-08), clamped to
 * the rows the new page has. Declare before the canvas mounts, so the
 * position is read before the canvas is destroyed.
 */
export function useGridPosition({
  memory,
  handle,
  selection,
  mounted,
  rows,
}: {
  memory: React.RefObject<PositionMemory>;
  handle: React.RefObject<GridHandle | undefined>;
  selection: React.RefObject<SelectionModel>;
  /** Mount count of the canvas; changes once it is ready. */
  mounted: number;
  /** Rows of the shown page. */
  rows: number;
}): void {
  useEffect(
    () => () => {
      const grid = handle.current;
      if (!grid) return;
      const { row, column } = selection.current;
      memory.current.position = {
        x: grid.viewport.x,
        y: grid.viewport.y,
        row,
        column,
      };
    },
    [memory, handle, selection],
  );
  useEffect(() => {
    const grid = handle.current;
    if (!mounted || !grid) return;
    const position = takeRestore(memory);
    if (!position) return;
    grid.scrollTo(position.x, position.y);
    if (rows > 0)
      grid.select?.(Math.min(position.row, rows - 1), position.column);
  }, [memory, handle, mounted, rows]);
}
