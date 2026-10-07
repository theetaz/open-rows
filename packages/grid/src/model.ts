// SPDX-License-Identifier: Apache-2.0
import type { light } from '@openrows/ui/tokens';

/** Fixed-width columns; pinned columns must form a leading prefix. */
export interface GridColumn {
  id: string;
  title: string;
  width: number;
  pinned: boolean;
  /** `end` right-aligns values, e.g. numbers; headers stay start-aligned. */
  align?: 'start' | 'end';
}
/** Inclusive logical ranges; never materialize the selected rows. */
export interface CellRange {
  r0: number;
  r1: number;
  c0: number;
  c1: number;
}
/** Focus and multiple disjoint ranges use zero-based model indices. */
export interface SelectionModel {
  row: number;
  column: number;
  ranges: readonly CellRange[];
  /** Fixed origin while extending a selection with Shift. */
  anchor?: { row: number; column: number };
}
/** Converted once on tile arrival, outside the painter; `empty` is an empty string shown as `EMPTY`. */
export interface DisplayCell {
  text: string;
  state: 'normal' | 'null' | 'empty' | 'changed' | 'deleted';
}
/** Mutable counters are sampled outside painting. */
export interface SourceMetrics {
  requested: number;
  hits: number;
  tileApplyMs: number;
  tileConvertMs: number;
  workerConvertMs: number;
  tileAllocations: number;
  arrayAllocations: number;
  cellAllocations: number;
}
/** Window fills asynchronously; cell lookup never starts I/O. */
export interface GridDataSource {
  readonly rows: number;
  readonly columns: readonly GridColumn[];
  readonly metrics: SourceMetrics;
  cell(row: number, column: number): DisplayCell | undefined;
  window(range: CellRange, direction: number): Promise<void>;
  subscribe(listener: () => void): () => void;
}
/** Tokens from @openrows/ui, resolved at mount for either renderer. */
export interface GridTheme {
  colours: typeof light;
  rowHeight: number;
  fontSize: number;
  fontFamily: string;
}
/** Logical target and CSS-pixel pointer anchor for a contextual action menu. */
export type GridContextTarget = {
  row: number;
  column: number;
  kind: 'cell' | 'row' | 'column';
  x: number;
  y: number;
};
/** Both renderers emit the same focus/selection events and terminal errors. */
export interface GridProps {
  source: GridDataSource;
  selection: SelectionModel;
  theme: GridTheme;
  onSelection(selection: SelectionModel): void;
  onError(error: unknown): void;
  /** Header click on a model column, e.g. to sort a table view server-side. */
  onHeader?(column: number): void;
  /** Optional pinned row-label column, excluded from data selection. */
  rowHeaderColumn?: number;
  striped?: boolean;
  onEdit?(cell: { row: number; column: number }): void;
  onContextMenu?(target: GridContextTarget): void;
}
/** Paint counters reset at each benchmark frame; placeholder draws flag blank frames. */
export interface PaintMetrics {
  milliseconds: number;
  placeholders: number;
  paints: number;
  scrollHandlerMs: number;
  reactCommitMs: number;
  canvasResizeMs: number;
  canvasResizeEvents: number;
  prepareMs: number;
  cellAllocations: number;
  arrayAllocations: number;
}
/** Imperative spike boundary; production integration is intentionally out of scope. */
export interface GridHandle {
  readonly metrics: PaintMetrics;
  readonly viewport: { x: number; y: number; width: number; height: number };
  scrollTo(x: number, y: number): void;
  /** Canvas can update paint settings without discarding loaded tiles. */
  setTheme?(theme: GridTheme): void;
  destroy(): void;
}
/** A renderer is only imported on demand after the native window is shown. */
export type MountGrid = (host: HTMLElement, props: GridProps) => GridHandle;
