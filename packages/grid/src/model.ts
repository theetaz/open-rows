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
  /** Booleans paint centred TRUE/FALSE; every kind shows an up/down stepper. */
  kind?: 'boolean' | 'enum' | 'temporal';
  /** A foreign key: non-NULL values paint a → arrow that calls `onLink`. */
  link?: boolean;
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
  state: 'normal' | 'null' | 'empty' | 'changed' | 'deleted' | 'inserted';
  /** A staged NULL, empty string or DEFAULT, painted muted like `null`/`empty`. */
  marker?: 'null' | 'empty' | 'default';
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
  /** Enter, double-click, a stepper click, or a typed printable `text` that seeds the editor. */
  onEdit?(cell: { row: number; column: number }, text?: string): void;
  /** A ⌃⌄ stepper click; falls back to `onEdit` when absent. */
  onStepper?(cell: { row: number; column: number }): void;
  onContextMenu?(target: GridContextTarget): void;
  /** The → arrow of a `link` column's cell was clicked (open the referenced row). */
  onLink?(cell: { row: number; column: number }): void;
  /** Native scroll, e.g. to keep an editor overlay aligned with its cell. */
  onScroll?(): void;
  /**
   * A header edge drag (live, then `done`) or a double-click auto-size. The
   * host stores the width in `source.columns` and calls `relayout`.
   */
  onColumnResize?(column: number, width: number, done: boolean): void;
  /** The row-number header was clicked: size every column to its content. */
  onFitColumns?(): void;
  /** A header dropped at index `to`; the host reorders and remounts or relayouts. */
  onColumnMove?(from: number, to: number): void;
}
/** Host-relative CSS-pixel rectangle of one cell. */
export interface CellRect {
  x: number;
  y: number;
  width: number;
  height: number;
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
  /** Re-read column kinds and alignment, then repaint. */
  refresh?(): void;
  /** Re-read columns and rows (widths, order, pins, a new row view), keeping scroll, selection and tiles. */
  relayout?(): void;
  /** Width that fits a column's title and the loaded values near the viewport. */
  fitWidth?(column: number): number;
  /** Where a cell is drawn now, relative to the host. */
  cellRect?(row: number, column: number): CellRect | undefined;
  /** Focus and reveal one cell, as if the user had clicked it. */
  select?(row: number, column: number): void;
  /** Return keyboard focus to the grid. */
  focus?(): void;
  /** Politely announce a state change, e.g. entering edit mode. */
  announce?(message: string): void;
  destroy(): void;
}
/** A renderer is only imported on demand after the native window is shown. */
export type MountGrid = (host: HTMLElement, props: GridProps) => GridHandle;
