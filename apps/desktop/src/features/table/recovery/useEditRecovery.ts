// SPDX-License-Identifier: Apache-2.0
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import {
  clearTableEdits,
  savedTableEdits,
  saveTableEdits,
} from '@openrows/ipc';
import type { SavedEditsDto } from '@openrows/ipc';
import { restored } from '../editHistory';
import type { StagedEdit } from '../editHistory';
import type { TableTargetRef } from '../model';
import type { RowEditing } from '../useRowEditing';
import type { TableViews } from '../useTableViews';
import { fromSaved, sameKey, toSaved } from './journal';

/** Journaled edits found for this table, or why the journal cannot be read. */
export type RecoveryOffer =
  | { kind: 'edits'; saved: SavedEditsDto; edits: readonly StagedEdit[] }
  | { kind: 'error'; message: string };

export type EditRecovery = {
  offer: RecoveryOffer | null;
  /** The table's key still matches the journal, so the edits can come back. */
  canRestore: boolean;
  restore: () => void;
  discard: () => void;
};

type Args = {
  tabId: string;
  target: TableTargetRef;
  views: TableViews;
  editing: RowEditing;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Keep the local journal of this table's staged edits (NFR-REL-01): written
 * on every mutation, cleared on commit or discard, and offered back when the
 * table opens with nothing staged.
 */
export function useEditRecovery({
  tabId,
  target,
  views,
  editing,
}: Args): EditRecovery {
  const [offer, setOffer] = useState<RecoveryOffer | null>(null);
  const { profileId, database, schema, table } = target;
  const { history } = editing;
  const current = history.current;
  const staged = current.length > 0;
  const journal = useEffectEvent((edits: readonly StagedEdit[]) => {
    // This session's edits supersede whatever an earlier run left behind.
    setOffer(null);
    const write = edits.length
      ? saveTableEdits(target, toSaved(history, editing.info?.key ?? []))
      : clearTableEdits(target);
    write.catch(editing.fail);
  });
  const last = useRef<readonly StagedEdit[] | null>(null);
  useEffect(() => {
    if (history.pending || history.uncertain) return;
    if (last.current !== null && last.current !== current) journal(current);
    last.current = current;
  }, [current, history.pending, history.uncertain]);
  useEffect(() => {
    let alive = true;
    const ref = { profileId, database, schema, table };
    savedTableEdits(ref).then(
      (saved) => {
        if (alive && saved?.changes.length)
          setOffer({ kind: 'edits', saved, edits: fromSaved(saved) });
      },
      (failure: unknown) => {
        if (alive) setOffer({ kind: 'error', message: message(failure) });
      },
    );
    return () => {
      alive = false;
    };
  }, [profileId, database, schema, table]);
  const shown = staged ? null : offer;
  const canRestore =
    shown?.kind === 'edits' &&
    !!editing.info &&
    sameKey(shown.saved.key, editing.info.key);
  return {
    offer: shown,
    canRestore,
    restore: () => {
      if (!canRestore) return;
      views.setEdits(tabId, () => restored(shown.edits));
      setOffer(null);
    },
    discard: () => {
      setOffer(null);
      clearTableEdits(target).catch(editing.fail);
    },
  };
}
