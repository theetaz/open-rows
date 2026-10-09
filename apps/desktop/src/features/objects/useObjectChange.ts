// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { objectChange } from '@openrows/ipc';
import type { ObjectChangeDto, ObjectReply } from '@openrows/ipc';
import { usePreview } from '../table/structure/forms/usePreview';
import type { PreviewState } from '../table/structure/forms/usePreview';

/** A reviewed change: what runs, its token and the name to type, if any. */
export type Prepared = Extract<ObjectReply, { kind: 'prepared' }>;

/** Where a change runs: the connection and the open database. */
export type ChangeScope = { profileId: string; database: string };

/** The live review of one change, and running it. */
export type ObjectChangeState = {
  preview: PreviewState<Prepared>;
  /** The name typed to confirm a guarded destructive change. */
  typed: string;
  setTyped: (typed: string) => void;
  busy: boolean;
  error: string | null;
  /** The reviewed statements may run now. */
  ready: boolean;
  run: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Review `change` as it is edited (no token: nothing runs), then run
 * exactly the reviewed statements with the review's token.
 */
export function useObjectChange(
  scope: ChangeScope,
  change: ObjectChangeDto | null,
  onDone: (change: ObjectChangeDto) => void,
): ObjectChangeState {
  const [typed, setTyped] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const key = change ? JSON.stringify([scope, change]) : null;
  const preview = usePreview<Prepared>(key, async () => {
    if (!change) throw new Error('Nothing to review');
    const reply = await objectChange({
      ...scope,
      change,
      confirmToken: null,
      confirmName: null,
    });
    if (reply.kind !== 'prepared') throw new Error('Nothing was reviewed');
    return reply;
  });
  const reviewed = preview.status === 'ready' ? preview : null;
  const named = !reviewed?.confirmName || typed === reviewed.confirmName;
  const ready = Boolean(change && reviewed && named && !busy);
  const run = (): void => {
    if (!ready || !change || !reviewed) return;
    setBusy(true);
    setError(null);
    objectChange({
      ...scope,
      change,
      confirmToken: reviewed.token,
      confirmName: reviewed.confirmName ? typed : null,
    })
      .then((reply) => {
        if (!alive.current) return;
        if (reply.kind === 'committed') onDone(change);
        else setError('The statements changed; review them again.');
      })
      .catch((failure: unknown) => {
        if (alive.current) setError(message(failure));
      })
      .finally(() => {
        if (alive.current) setBusy(false);
      });
  };
  return { preview, typed, setTyped, busy, error, ready, run };
}
