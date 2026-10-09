// SPDX-License-Identifier: Apache-2.0
import type { TableQueryDto } from '@openrows/ipc';
import { isActive } from '../query/run';
import type { ResultMeta } from '../query/run';
import type { TableObject } from '../query/tabs';
import {
  arrangedQuery,
  countKey,
  filterRow,
  nextSort,
  pageKey,
  tableQuery,
  tableTarget,
  toFilters,
} from './model';
import { hideRowId } from './row';
import { arrange } from '../../results/columnLayout';
import type { EngineDto } from '@openrows/ipc';
import { useTableLayout } from './useTableLayout';
import type { TableLayout } from './useTableLayout';
import type { ContextMenuProps } from './contextMenuActions';
import type { FilterRow, TableView, Total, ViewMode } from './model';
import type { TableEntry } from './entries';
import type { TableViews } from './useTableViews';
import { useTrigger } from './useTrigger';
import { usePresentation } from './usePresentation';
import { withSavedFilter } from './presentation';
import type { SavedFilter } from './presentation';

type Args = {
  tabId: string;
  table: TableObject;
  profileId: string;
  /** The session pages run on, or `null` when they cannot run. */
  sessionId: string | null;
  views: TableViews;
  /** A keyless table's rowid: selected for edits, never shown. */
  rowId: string | null;
  /** Engine of the connection: quoting for copied SQL. */
  engine: EngineDto;
};

export { useTrigger } from './useTrigger';

/** What a table tab's controls can do. */
export type TableActions = {
  sort: (result: ResultMeta, column: number) => void;
  page: (direction: -1 | 1) => void;
  /** Apply every enabled filter row, or only row `only`. */
  apply: (only?: number) => void;
  clear: () => void;
  combine: (combinator: TableView['combinator']) => void;
  editRow: (id: number, patch: Partial<FilterRow>) => void;
  removeRow: (id: number) => void;
  addRow: (after?: number) => void;
  toggleFilters: () => void;
  toggleColumns: () => void;
  display: (setting: 'rowNumbers' | 'striped') => void;
  toggleHidden: (name: string) => void;
  mode: (mode: ViewMode) => void;
  count: () => void;
  /** Load the current page again (Refresh). */
  reload: () => void;
  /** Load a saved filter into the bar and apply it. */
  applySaved: (saved: SavedFilter) => void;
  /** Show the focused row as a vertical form, or the grid again. */
  toggleForm: () => void;
};

/** A table tab's derived state and actions. */
export type TableTabState = {
  entry: TableEntry;
  view: TableView;
  /** Catalog column names, once the structure has loaded. */
  columns: readonly string[];
  query: TableQueryDto;
  result: ResultMeta | undefined;
  total: Total;
  actions: TableActions;
  cancelPage: () => void;
  cancelCount: () => void;
  /** Column widths, order and pins; the page selects in that order. */
  layout: TableLayout;
  /** What the grid's context menu changes. */
  menu: Pick<ContextMenuProps, 'grid' | 'changeView'>;
  /** The table's presentation key (remembered view, saved filters, seeds). */
  storageKey: string;
};

/** View state, derived request and actions of one table tab. */
export function useTableTab({
  tabId,
  table,
  profileId,
  sessionId,
  views,
  rowId,
  engine,
}: Args): TableTabState {
  const entry = views.entryOf(tabId);
  const columnLayout = useTableLayout(table, profileId, engine);
  const { layout } = columnLayout;
  const { structure } = entry;
  const target = tableTarget(table, profileId);
  const presentation = usePresentation({
    tabId,
    target,
    views,
    view: entry.view,
  });
  const { view, key: storageKey } = presentation;
  const columns =
    structure.status === 'ready'
      ? structure.data.columns.map((column) => column.name)
      : [];
  const query = arrangedQuery(
    tableQuery(target, view, columns, rowId),
    columns,
    arrange(layout, columns),
    view.hidden,
  );
  const key = sessionId ? pageKey(query, view, sessionId) : null;
  useTrigger(structure.status === 'idle' ? 'load' : null, () => {
    views.loadStructure(tabId, target);
  });
  useTrigger(key !== entry.loaded ? key : null, () => {
    if (key && sessionId)
      views.loadPage(tabId, key, {
        sessionId,
        query,
        offset: view.offset,
        limit: view.limit,
      });
  });
  const change = (update: (view: TableView) => TableView): void => {
    views.setView(tabId, update);
  };
  return {
    entry: entry.view === view ? entry : { ...entry, view },
    view,
    columns,
    query,
    result: hideRowId(entry.run.results.at(-1), rowId),
    total: total(entry, query),
    actions: {
      ...actions({ change, columns, entry }, () => {
        if (sessionId)
          views.count(tabId, countKey(query), { sessionId, query });
      }),
      reload: () => {
        change(reloaded);
      },
      applySaved: (saved) => {
        change((current) => withSavedFilter(current, saved));
      },
      toggleForm: () => {
        change((current) => ({ ...current, form: !current.form }));
      },
    },
    cancelPage: () => {
      views.cancelPage(tabId);
    },
    cancelCount: () => {
      views.cancelCount(tabId);
    },
    layout: columnLayout,
    menu: { grid: columnLayout.grid, changeView: change },
    storageKey,
  };
}

/** The exact count for the current filters, else the estimate when unfiltered. */
function total(entry: TableEntry, query: TableQueryDto): Total {
  const { count, structure } = entry;
  const exact =
    count.status === 'done' && count.key === countKey(query)
      ? count.rows
      : null;
  const estimate =
    structure.status === 'ready' && !query.filters.length
      ? structure.data.rowEstimate
      : null;
  return { exact, estimate };
}

type Context = {
  change: (update: (view: TableView) => TableView) => void;
  columns: readonly string[];
  entry: TableEntry;
};

function actions(
  { change, columns, entry }: Context,
  count: () => void,
): Omit<TableActions, 'reload' | 'applySaved' | 'toggleForm'> {
  const editRows = (
    update: (rows: readonly FilterRow[]) => FilterRow[],
  ): void => {
    change((view) => ({ ...view, rows: update(view.rows) }));
  };
  const addRow = (after?: number): void => {
    change((view) => {
      const row = filterRow(view.nextRow, columns[0] ?? '');
      const index = view.rows.findIndex((item) => item.id === after);
      const rows = [...view.rows];
      rows.splice(index < 0 ? rows.length : index + 1, 0, row);
      return { ...view, rows, nextRow: view.nextRow + 1, filtersOpen: true };
    });
  };
  return {
    sort: (result: ResultMeta, column: number) => {
      const name = result.columns[column]?.name;
      if (name)
        change((view) => ({
          ...view,
          sort: nextSort(view.sort, name),
          offset: 0,
        }));
    },
    page: (direction: -1 | 1) => {
      if (direction > 0 && !canNext(entry)) return;
      change((view) => ({
        ...view,
        offset: Math.max(0, view.offset + direction * view.limit),
      }));
    },
    apply: (only?: number) => {
      change((view) => ({
        ...view,
        applied: toFilters(view.rows, only),
        offset: 0,
      }));
    },
    clear: () => {
      change((view) => ({ ...view, rows: [], applied: [], offset: 0 }));
    },
    combine: (combinator: TableView['combinator']) => {
      change((view) => ({ ...view, combinator }));
    },
    editRow: (id: number, patch: Partial<FilterRow>) => {
      editRows((rows) => updateFilter(rows, id, patch));
    },
    removeRow: (id: number) => {
      editRows((rows) => rows.filter((row) => row.id !== id));
    },
    addRow,
    toggleFilters: () => {
      if (!entry.view.filtersOpen && !entry.view.rows.length) addRow();
      else change((view) => ({ ...view, filtersOpen: !view.filtersOpen }));
    },
    toggleColumns() {
      change((view) => ({ ...view, columnsOpen: !view.columnsOpen }));
    },
    display: (setting) => {
      change((view) => ({ ...view, [setting]: !view[setting] }));
    },
    toggleHidden: (name: string) => {
      change((view) => ({
        ...view,
        hidden: view.hidden.includes(name)
          ? view.hidden.filter((item) => item !== name)
          : [...view.hidden, name],
      }));
    },
    mode: (mode: ViewMode) => {
      change((view) => ({ ...view, mode }));
    },
    count,
  };
}

/** The same view, asked to load its page again. */
function reloaded(view: TableView): TableView {
  return { ...view, revision: view.revision + 1 };
}

/** A next page exists when the shown page is full and finished loading. */
export function canNext(entry: TableEntry): boolean {
  const result = entry.run.results.at(-1);
  return (
    !isActive(entry.run) && !!result?.done && result.rows >= entry.view.limit
  );
}

function updateFilter(
  rows: readonly FilterRow[],
  id: number,
  patch: Partial<FilterRow>,
): FilterRow[] {
  return rows.map((row) => (row.id === id ? { ...row, ...patch } : row));
}
