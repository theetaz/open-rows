// SPDX-License-Identifier: Apache-2.0
import type { TableStructureDto } from '@openrows/ipc';
import { idleRun, runReducer } from '../query/run';
import type { RunAction, RunState } from '../query/run';
import { initialView } from './model';
import type { TableView } from './model';

/** A structure read: loading, loaded or failed. */
export type StructureState =
  | { status: 'idle' | 'loading' }
  | { status: 'ready'; data: TableStructureDto }
  | { status: 'error'; message: string };

/** An exact count for the filters identified by `key`. */
export type CountState =
  | { status: 'idle' }
  | { status: 'counting'; key: string; ticket: string }
  | { status: 'done'; key: string; rows: number }
  | { status: 'error'; key: string; message: string };

/** Everything a table tab keeps across tab switches; never row data. */
export type TableEntry = {
  view: TableView;
  run: RunState;
  /** Key of the page last requested, so remounting does not reload. */
  loaded: string | null;
  /** SQL of the page last requested. */
  sql: string | null;
  structure: StructureState;
  count: CountState;
};

export const emptyEntry: TableEntry = {
  view: initialView,
  run: idleRun,
  loaded: null,
  sql: null,
  structure: { status: 'idle' },
  count: { status: 'idle' },
};

export type EntryAction =
  | { type: 'view'; change: (view: TableView) => TableView }
  | { type: 'run'; action: RunAction }
  | { type: 'requested'; key: string }
  | { type: 'sql'; sql: string }
  | { type: 'structure'; structure: StructureState }
  | { type: 'count'; count: CountState };

function entryReducer(entry: TableEntry, action: EntryAction): TableEntry {
  switch (action.type) {
    case 'view':
      return { ...entry, view: action.change(entry.view) };
    case 'run':
      return { ...entry, run: runReducer(entry.run, action.action) };
    case 'requested':
      return { ...entry, loaded: action.key, sql: null };
    case 'sql':
      return { ...entry, sql: action.sql };
    case 'structure':
      return { ...entry, structure: action.structure };
    case 'count':
      return { ...entry, count: action.count };
  }
}

type Entries = Readonly<Record<string, TableEntry | undefined>>;

/** Per-tab entries; a `null` action forgets the tab. */
export function entriesReducer(
  entries: Entries,
  { tabId, action }: { tabId: string; action: EntryAction | null },
): Entries {
  if (!action)
    return Object.fromEntries(
      Object.entries(entries).filter(([id]) => id !== tabId),
    );
  return {
    ...entries,
    [tabId]: entryReducer(entries[tabId] ?? emptyEntry, action),
  };
}
