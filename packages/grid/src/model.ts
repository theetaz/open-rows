// SPDX-License-Identifier: Apache-2.0
import type { light } from '@openrows/ui/tokens';

/** Fixed-width columns; pinned columns must form a leading prefix. */
export interface GridColumn {
  id: string;
  title: string;
  width: number;
  pinned: boolean;
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
}
/** Converted once on tile arrival, outside the painter. */
export interface DisplayCell {
  text: string;
  state: 'normal' | 'null' | 'changed' | 'deleted';
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
/** Both renderers emit the same focus/selection events and terminal errors. */
export interface GridProps {
  source: GridDataSource;
  selection: SelectionModel;
  theme: GridTheme;
  onSelection(selection: SelectionModel): void;
  onError(error: unknown): void;
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
  destroy(): void;
}
/** A renderer is only imported on demand after the native window is shown. */
export type MountGrid = (host: HTMLElement, props: GridProps) => GridHandle;
