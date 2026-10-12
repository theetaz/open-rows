// SPDX-License-Identifier: Apache-2.0
import { localSetting } from './localSetting';

/** `quiet` scrollbars fade until hovered (macOS-like); `always` keeps them visible. */
export type GridScrollbars = 'quiet' | 'always';
/** `estimate` shows the planner's ~count; `exact` also counts after each load. */
export type RowCountMode = 'estimate' | 'exact';

/** Table presentation defaults of Settings ▸ Tables (FR-GRID-12). */
type TableSettings = {
  /** Alternate row backgrounds unless a tab toggles them. */
  striped: boolean;
  scrollbars: GridScrollbars;
  rowCount: RowCountMode;
};

const defaultTableSettings: TableSettings = {
  striped: true,
  scrollbars: 'quiet',
  rowCount: 'estimate',
};

/** Key in web storage (and `tables` in settings.json). */
export const TABLES_KEY = 'openrows.tables';

/** Validate stored settings field by field, keeping defaults otherwise. */
export function parseTableSettings(stored: unknown): TableSettings {
  if (typeof stored !== 'object' || stored === null)
    return defaultTableSettings;
  const value = stored as Record<string, unknown>;
  return {
    striped:
      typeof value.striped === 'boolean'
        ? value.striped
        : defaultTableSettings.striped,
    scrollbars: value.scrollbars === 'always' ? 'always' : 'quiet',
    rowCount: value.rowCount === 'exact' ? 'exact' : 'estimate',
  };
}

export const tableSettings = localSetting(TABLES_KEY, parseTableSettings);

/** Change some table settings. */
export function updateTableSettings(change: Partial<TableSettings>): void {
  tableSettings.set({ ...tableSettings.get(), ...change });
}

/** Reflect the scrollbar style on the root element for the grid CSS. */
export function applyTableSettings(settings: TableSettings): void {
  document.documentElement.dataset.gridScrollbars = settings.scrollbars;
}
