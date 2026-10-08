// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { CommandError, tableAlter } from '@openrows/ipc';
import type {
  AlterReply,
  SchemaChangeDto,
  TableDetailDto,
} from '@openrows/ipc';
import { emptyHistory, travel } from '../editHistory';
import type { TableTargetRef } from '../model';
import type { TableViews } from '../useTableViews';
import { replaceStaged, stageSchema, unstage } from './schemaChanges';
import type { SchemaHistory } from './schemaChanges';

type Prepared = Extract<AlterReply, { kind: 'prepared' }>;

type Args = {
  tabId: string;
  target: TableTargetRef;
  sessionId: string | null;
  views: TableViews;
  /** The table was renamed by a committed change set. */
  onRenamed?: ((name: string) => void) | undefined;
};

/** Staged structure changes of one table tab, their review and commit. */
export type SchemaEditing = {
  history: SchemaHistory;
  detail: TableDetailDto | null;
  /** Structure changes can be staged and committed now. */
  enabled: boolean;
  busy: boolean;
  error: string | null;
  review: Prepared | null;
  stage: (change: SchemaChangeDto) => void;
  replace: (id: string, change: SchemaChangeDto) => void;
  unstage: (id: string) => void;
  undo: () => void;
  redo: () => void;
  discard: () => void;
  /** Without a token: prepare the review; with it: commit. */
  commit: (token?: string, confirmName?: string) => void;
  close: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

export function useSchemaEditing(args: Args): SchemaEditing {
  const { tabId, views } = args;
  const entry = views.entryOf(tabId);
  const history = entry.schema;
  const detail = entry.detail.status === 'ready' ? entry.detail.data : null;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [review, setReview] = useState<Prepared | null>(null);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const change = (update: (current: SchemaHistory) => SchemaHistory): void => {
    views.setSchema(tabId, update);
    setReview(null);
    setError(null);
  };
  const enabled = Boolean(
    detail?.editable && args.sessionId && !history.uncertain,
  );
  return {
    history,
    detail,
    enabled,
    busy: busy || history.pending,
    error,
    review,
    stage: (next) => {
      if (detail)
        change((current) =>
          stageSchema(current, next, { detail, table: args.target.table }),
        );
    },
    replace: (id, next) => {
      change((current) => replaceStaged(current, id, next));
    },
    unstage: (id) => {
      change((current) => unstage(current, id));
    },
    undo: () => {
      change((current) => travel(current, 'undo'));
    },
    redo: () => {
      change((current) => travel(current, 'redo'));
    },
    discard: () => {
      change(() => emptyHistory);
      if (history.uncertain) reload(views, tabId);
    },
    commit: (token, confirmName) => {
      if (!enabled || busy || history.pending || !history.current.length)
        return;
      setBusy(true);
      setError(null);
      void commitChanges({ args, history, token, confirmName, setReview })
        .catch((failure: unknown) => {
          if (alive.current) setError(message(failure));
        })
        .finally(() => {
          if (alive.current) setBusy(false);
        });
    },
    close: () => {
      if (!busy) {
        setReview(null);
        setError(null);
      }
    },
  };
}

function reload(views: TableViews, tabId: string): void {
  views.setView(tabId, (view) => ({ ...view, revision: view.revision + 1 }));
}

async function commitChanges({
  args,
  history,
  token,
  confirmName,
  setReview,
}: {
  args: Args;
  history: SchemaHistory;
  token: string | undefined;
  confirmName: string | undefined;
  setReview: (review: Prepared | null) => void;
}): Promise<void> {
  const { views, tabId, sessionId } = args;
  if (!sessionId) return;
  const changes = history.current.map((item) => item.change);
  const pending = (value: boolean): void => {
    views.setSchema(tabId, (current) => ({ ...current, pending: value }));
  };
  pending(true);
  try {
    const reply = await tableAlter({
      sessionId,
      target: args.target,
      changes,
      confirmToken: token ?? null,
      confirmName: confirmName ?? null,
    });
    if (reply.kind === 'prepared') {
      setReview(reply);
      return;
    }
    views.setSchema(tabId, () => emptyHistory);
    setReview(null);
    const renamed = changes.find((change) => change.kind === 'renameTable');
    if (renamed?.kind === 'renameTable') args.onRenamed?.(renamed.name);
    reload(views, tabId);
  } catch (failure) {
    if (failure instanceof CommandError && failure.code === '08007')
      views.setSchema(tabId, (current) => ({ ...current, uncertain: true }));
    throw failure;
  } finally {
    pending(false);
  }
}
