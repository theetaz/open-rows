// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useReducer, useRef } from 'react';
import {
  CommandError,
  cancelQuery,
  releaseResults,
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
import { countRequests } from './countRequests';

/** Staged and in-flight work across every table tab, for the quit guard. */
export type ChangeSummary = {
  pending: boolean;
  uncertain: boolean;
  staged: number;
};

/** State and IPC actions of every open table tab. */
export type TableViews = {
  hasChanges: () => boolean;
  summary: () => ChangeSummary;
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
 * after a cancel) are dropped and their results freed. The previous page's
 * results (`stale`) stay readable until this page finishes, so the grid can
 * keep showing them while it loads.
 */
function startPage(
  send: Send,
  latest: Record<string, string | undefined>,
  [tabId, key, request, stale]: [string, string, PageRequest, string[]],
): void {
  latest[tabId] = key;
  const current = (): boolean => latest[tabId] === key;
  let pending = stale;
  const drop = (): void => {
    free(pending);
    pending = [];
  };
  send(tabId, { type: 'requested', key });
  send(tabId, { type: 'run', action: { type: 'begin' } });
  tablePage(request, (event) => {
    if (!current() || event.type === 'finished') drop();
    if (current())
      send(tabId, { type: 'run', action: { type: 'event', event } });
    else if (event.type === 'resultBegin') free([event.resultId]);
  }).then(
    (reply) => {
      if (!current()) {
        drop();
        return;
      }
      send(tabId, { type: 'sql', sql: reply.sql });
      send(tabId, {
        type: 'run',
        action: { type: 'started', queryId: reply.queryId },
      });
    },
    (error: unknown) => {
      drop();
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

/** Count ownership follows the workspace lifetime rather than individual tab mounts. */
function useCounts(
  dispatch: React.Dispatch<{ tabId: string; action: EntryAction | null }>,
): ReturnType<typeof countRequests> {
  const counts = useMemo(
    () =>
      countRequests((tabId, count) => {
        dispatch({ tabId, action: { type: 'count', count } });
      }),
    [dispatch],
  );
  useEffect(
    () => () => {
      counts.clear();
    },
    [counts],
  );
  return counts;
}

/** Table tab state lives here (above the tab) so it survives tab switches. */
export function useTableViews(): TableViews {
  const [entries, dispatch] = useReducer(entriesReducer, {});
  const counts = useCounts(dispatch);
  const latest = useRef<Record<string, string | undefined>>({});
  const send: Send = (tabId, action) => {
    dispatch({ tabId, action });
  };
  const summary = (): ChangeSummary => {
    const result = { pending: false, uncertain: false, staged: 0 };
    for (const entry of Object.values(entries)) {
      if (!entry) continue;
      result.pending ||= entry.edits.pending || entry.schema.pending;
      result.uncertain ||= Boolean(
        entry.edits.uncertain || entry.schema.uncertain,
      );
      result.staged += entry.edits.current.length + entry.schema.current.length;
    }
    return result;
  };
  return {
    hasChanges: () => {
      const { pending, staged } = summary();
      return pending || staged > 0;
    },
    summary,
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
      startPage(send, latest.current, [
        tabId,
        key,
        request,
        pageIds(entries[tabId]),
      ]);
    },
    cancelPage: (tabId) => {
      const queryId = entries[tabId]?.run.queryId;
      if (queryId) cancelQuery(queryId).catch(() => undefined);
    },
    loadStructure: (tabId, target) => {
      readStructure(send, tabId, target);
    },
    count: (tabId, key, request) => {
      counts.start(tabId, key, request);
    },
    cancelCount: (tabId) => {
      counts.cancel(tabId);
    },
    release: (tabId) => {
      counts.release(tabId);
      free(pageIds(entries[tabId]));
      latest.current[tabId] = undefined;
      dispatch({ tabId, action: null });
    },
  };
}
