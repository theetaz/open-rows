// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useRef, useState } from 'react';
import { tableEditInfo } from '@openrows/ipc';
import type { EditInfoDto } from '@openrows/ipc';
import type { TableTargetRef } from './model';
const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);
/** Ignore completed reads after unmount and expose failures to the user. */
export function useEditIo(target: TableTargetRef): EditIo {
  const [info, setInfo] = useState<EditInfoDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const alive = useRef(true);
  const locked = useRef(false);
  const { profileId, database, schema, table } = target;
  const stableTarget = useMemo(
    () => ({ profileId, database, schema, table }),
    [profileId, database, schema, table],
  );
  useEffect(() => {
    let current = true;
    alive.current = true;
    tableEditInfo(stableTarget).then(
      (value) => {
        if (current) setInfo(value);
      },
      (failure: unknown) => {
        if (current) setError(message(failure));
      },
    );
    return () => {
      current = false;
      alive.current = false;
    };
  }, [stableTarget]);
  const run = async (action: () => Promise<void>): Promise<void> => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (failure) {
      if (alive.current) setError(message(failure));
    } finally {
      locked.current = false;
      if (alive.current) setBusy(false);
    }
  };
  return { info, error, busy, alive, run };
}

export type EditIo = {
  info: EditInfoDto | null;
  error: string | null;
  busy: boolean;
  alive: React.RefObject<boolean>;
  run: (action: () => Promise<void>) => Promise<void>;
};
