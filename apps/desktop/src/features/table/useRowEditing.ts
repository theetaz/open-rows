// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { CommandError, tableEdit } from '@openrows/ipc';
import type { EditReply, EditInfoDto } from '@openrows/ipc';
import type { ResultMeta } from '../query/run';
import type { TableTargetRef } from './model';
import type { TableViews } from './useTableViews';
import type { StagedEdit, EditHistory } from './editHistory';
import { emptyEdits, stage, travel } from './editHistory';
import { editDraft } from './editingIo';
import type { EditIo } from './useEditIo';
import { useEditIo } from './useEditIo';
import { quickEdit } from './quickEdits';
import type { QuickEditAction } from './quickEdits';

type Args = {
  tabId: string;
  target: TableTargetRef;
  sessionId: string | null;
  views: TableViews;
  offset: number;
};
/** Exact row snapshots and staged changes survive tab switches in TableViews. */
export function useRowEditing(args: Args): RowEditing {
  const io = useEditIo(args.target);
  const [draft, setDraft] = useState<StagedEdit | null>(null);
  const [review, setReview] = useState<Extract<
    EditReply,
    { kind: 'prepared' }
  > | null>(null);
  const history = args.views.entryOf(args.tabId).edits;
  const change = (edit: StagedEdit): void => {
    args.views.setEdits(args.tabId, (current) => stage(current, edit));
    setDraft(null);
    setReview(null);
  };
  const open = (
    result: ResultMeta,
    cell: { row: number; column: number },
  ): void => {
    openDraft({ args, io, history, result, cell, setDraft });
  };
  const commit = (token?: string): void => {
    if (!args.sessionId || history.pending || history.uncertain || io.busy)
      return;
    commitChanges({ args, history, io, token, setReview, setDraft });
  };
  return {
    editStaged: (edit) => {
      setDraft(edit.kind === 'delete' ? { ...edit, kind: 'update' } : edit);
    },
    info: io.info,
    error: io.error,
    busy: io.busy || history.pending,
    history,
    draft,
    review,
    open,
    quick: (result, cell, action) => {
      quickEdit({ ...args, io, history, result, cell, action, stage: change });
    },
    change,
    commit,
    close: () => {
      if (!io.busy) {
        setDraft(null);
        setReview(null);
      }
    },
    insert: () => {
      setDraft(newDraft());
    },
    discard: () => {
      args.views.setEdits(args.tabId, () => emptyEdits);
      if (history.uncertain) args.views.setView(args.tabId, refreshView);
      setReview(null);
    },
    undo: () => {
      args.views.setEdits(args.tabId, (current) => travel(current, 'undo'));
    },
    redo: () => {
      args.views.setEdits(args.tabId, (current) => travel(current, 'redo'));
    },
  };
}
function openDraft({
  args,
  io,
  history,
  result,
  cell,
  setDraft,
}: {
  args: Args;
  io: EditIo;
  history: EditHistory;
  result: ResultMeta;
  cell: { row: number; column: number };
  setDraft: (draft: StagedEdit) => void;
}): void {
  if (
    !io.alive.current ||
    !io.info ||
    io.info.readOnly ||
    !io.info.key.length ||
    history.pending ||
    history.uncertain
  )
    return;
  const info = io.info;
  void io.run(async () => {
    const next = await editDraft({
      info,
      result,
      row: cell.row,
      offset: args.offset,
      target: args.target,
      history,
    });
    const focusField = result.columns[cell.column]?.name;
    if (io.alive.current) setDraft(focusField ? { ...next, focusField } : next);
  });
}
export type RowEditing = {
  info: EditInfoDto | null;
  error: string | null;
  busy: boolean;
  history: EditHistory;
  draft: StagedEdit | null;
  review: Prepared | null;
  open: (result: ResultMeta, cell: { row: number; column: number }) => void;
  quick: (
    result: ResultMeta,
    cell: { row: number; column: number },
    action: QuickEditAction,
  ) => void;
  change: (edit: StagedEdit) => void;
  commit: (token?: string) => void;
  close: () => void;
  insert: () => void;
  discard: () => void;
  undo: () => void;
  redo: () => void;
  editStaged: (edit: StagedEdit) => void;
};
type Prepared = Extract<EditReply, { kind: 'prepared' }>;
function commitChanges({
  args,
  history,
  io,
  token,
  setReview,
  setDraft,
}: {
  args: Args;
  history: EditHistory;
  io: EditIo;
  token: string | undefined;
  setReview: (review: Prepared | null) => void;
  setDraft: (draft: StagedEdit | null) => void;
}): void {
  if (!args.sessionId) return;
  const request = {
    sessionId: args.sessionId,
    target: args.target,
    changes: history.current.map(({ kind, original, values }) => ({
      kind,
      original,
      values,
    })),
    confirmToken: token ?? null,
  };
  args.views.setEdits(args.tabId, (current) => ({
    ...current,
    pending: true,
  }));
  void io.run(async () => {
    try {
      const reply = await tableEdit(request);
      if (reply.kind === 'prepared') {
        if (io.alive.current) setReview(reply);
        return;
      }
      args.views.setEdits(args.tabId, () => emptyEdits);
      args.views.setView(args.tabId, (view) => ({
        ...view,
        revision: view.revision + 1,
      }));
      if (io.alive.current) {
        setReview(null);
        setDraft(null);
      }
    } catch (error) {
      if (error instanceof CommandError && error.code === '08007')
        args.views.setEdits(args.tabId, (current) => ({
          ...current,
          uncertain: true,
        }));
      throw error;
    } finally {
      args.views.setEdits(args.tabId, (current) => ({
        ...current,
        pending: false,
      }));
    }
  });
}

function newDraft(): StagedEdit {
  return {
    id: crypto.randomUUID(),
    number: 0,
    kind: 'insert',
    original: {},
    values: {},
  };
}

function refreshView(
  view: import('./model').TableView,
): import('./model').TableView {
  return { ...view, revision: view.revision + 1 };
}
