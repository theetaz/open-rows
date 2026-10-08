// SPDX-License-Identifier: Apache-2.0
import { useReducer, useRef } from 'react';
import {
  CommandError,
  cancelQuery,
  releaseResults,
  tableCount,
  tableDetail,
  tablePage,
  tableStructure,
} from '@openrows/ipc';
import type { PageRequest, TableQueryDto } from '@openrows/ipc';
import { isActive } from '../query/run';
import { emptyEntry, entriesReducer } from './entries';
import type { EntryAction, TableEntry } from './entries';
import type { EditHistory, History } from './editHistory';
import type { StagedSchema } from './structure/schemaChanges';
import type { TableTargetRef, TableView } from './model';

/** State and IPC actions of every open table tab. */
export type TableViews = {
  hasChanges: () => boolean;
  entryOf: (tabId: string) => TableEntry;
  setEdits: (
    tabId: string,
    change: (edits: EditHistory) => EditHistory,
  ) => void;
  setView: (tabId: string, change: (view: TableView) => TableView) => void;
  setSchema: (
    tabId: string,
    change: (schema: History<StagedSchema>) => History<StagedSchema>,
  ) => void;
  /** Read the Structure/Index/Triggers/DDL detail of the tab's table. */
  loadDetail: (tabId: string, target: TableTargetRef) => void;
  /** Run one page; `key` identifies it so a remount does not reload. */
  loadPage: (tabId: string, key: string, request: PageRequest) => void;
  cancelPage: (tabId: string) => void;
  loadStructure: (tabId: string, target: TableTargetRef) => void;
  /** Exact count of the rows `query` matches; `key` names its filters. */
  count: (
    tabId: string,
    key: string,
    request: { sessionId: string; query: TableQueryDto },
  ) => void;
  cancelCount: (tabId: string) => void;
  /** Forget a closed tab and free its page in the core. */
  release: (tabId: string) => void;
};

type Send = (tabId: string, action: EntryAction) => void;

function failure(error: unknown): { message: string; code: string | null } {
  if (error instanceof CommandError)
    return { message: error.message, code: error.code };
  return {
    message: error instanceof Error ? error.message : String(error),
    code: null,
  };
}

/** Free result memory; a failure means the core already dropped it. */
function free(ids: readonly string[]): void {
  if (ids.length) releaseResults(ids).catch(() => undefined);
}

function pageIds(entry: TableEntry | undefined): string[] {
  return entry?.run.results.map((result) => result.id) ?? [];
}

/**
 * Start a page. Events of an older page of the same tab (still finishing
 * after a cancel) are dropped and their results freed.
 */
function startPage(
  send: Send,
  latest: Record<string, string | undefined>,
  [tabId, key, request]: [string, string, PageRequest],
): void {
  latest[tabId] = key;
  const current = (): boolean => latest[tabId] === key;
  send(tabId, { type: 'requested', key });
  send(tabId, { type: 'run', action: { type: 'begin' } });
  tablePage(request, (event) => {
    if (current())
      send(tabId, { type: 'run', action: { type: 'event', event } });
    else if (event.type === 'resultBegin') free([event.resultId]);
  }).then(
    (reply) => {
      if (!current()) return;
      send(tabId, { type: 'sql', sql: reply.sql });
      send(tabId, {
        type: 'run',
        action: { type: 'started', queryId: reply.queryId },
      });
    },
    (error: unknown) => {
      if (current())
        send(tabId, {
          type: 'run',
          action: { type: 'rejected', ...failure(error) },
        });
    },
  );
}

function readStructure(
  send: Send,
  tabId: string,
  target: TableTargetRef,
): void {
  send(tabId, { type: 'structure', structure: { status: 'loading' } });
  tableStructure(target).then(
    (data) => {
      send(tabId, { type: 'structure', structure: { status: 'ready', data } });
    },
    (error: unknown) => {
      send(tabId, {
        type: 'structure',
        structure: { status: 'error', message: failure(error).message },
      });
    },
  );
}

function readDetail(send: Send, tabId: string, target: TableTargetRef): void {
  send(tabId, { type: 'detail', detail: { status: 'loading' } });
  tableDetail(target).then(
    (data) => {
      send(tabId, { type: 'detail', detail: { status: 'ready', data } });
    },
    (error: unknown) => {
      send(tabId, {
        type: 'detail',
        detail: { status: 'error', message: failure(error).message },
      });
    },
  );
}

let tickets = 0;

function startCount(
  send: Send,
  [tabId, key, request]: [
    string,
    string,
    { sessionId: string; query: TableQueryDto },
  ],
): void {
  tickets += 1;
  const ticket = `count-${String(tickets)}`;
  send(tabId, { type: 'count', count: { status: 'counting', key, ticket } });
  tableCount({ ...request, ticket }).then(
    (reply) => {
      send(tabId, {
        type: 'count',
        count:
          reply.kind === 'counted' && reply.rows !== null
            ? { status: 'done', key, rows: reply.rows }
            : { status: 'idle' },
      });
    },
    (error: unknown) => {
      send(tabId, {
        type: 'count',
        count: { status: 'error', key, message: failure(error).message },
      });
    },
  );
}

/** Table tab state lives here (above the tab) so it survives tab switches. */
export function useTableViews(): TableViews {
  const [entries, dispatch] = useReducer(entriesReducer, {});
  const latest = useRef<Record<string, string | undefined>>({});
  const send: Send = (tabId, action) => {
    dispatch({ tabId, action });
  };
  return {
    hasChanges: () =>
      Object.values(entries).some(
        (entry) =>
          entry &&
          (entry.edits.current.length > 0 ||
            entry.edits.pending ||
            entry.schema.current.length > 0 ||
            entry.schema.pending),
      ),
    entryOf: (tabId) => entries[tabId] ?? emptyEntry,
    setEdits: (tabId, change) => {
      send(tabId, { type: 'edits', change });
    },
    setView: (tabId, change) => {
      send(tabId, { type: 'view', change });
    },
    setSchema: (tabId, change) => {
      send(tabId, { type: 'schema', change });
    },
    loadDetail: (tabId, target) => {
      readDetail(send, tabId, target);
    },
    loadPage: (tabId, key, request) => {
      const previous = entries[tabId]?.run;
      if (previous?.queryId && isActive(previous))
        cancelQuery(previous.queryId).catch(() => undefined);
      free(pageIds(entries[tabId]));
      startPage(send, latest.current, [tabId, key, request]);
    },
    cancelPage: (tabId) => {
      const queryId = entries[tabId]?.run.queryId;
      if (queryId) cancelQuery(queryId).catch(() => undefined);
    },
    loadStructure: (tabId, target) => {
      readStructure(send, tabId, target);
    },
    count: (tabId, key, request) => {
      startCount(send, [tabId, key, request]);
    },
    cancelCount: (tabId) => {
      const state = entries[tabId]?.count;
      if (state?.status === 'counting')
        cancelQuery(state.ticket).catch(() => undefined);
    },
    release: (tabId) => {
      free(pageIds(entries[tabId]));
      latest.current[tabId] = undefined;
      dispatch({ tabId, action: null });
    },
  };
}
