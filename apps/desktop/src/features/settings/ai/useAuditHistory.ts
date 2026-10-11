// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { listAiAudit, purgeAiAudit } from '@openrows/ipc';
import type { AuditEntry } from '@openrows/ipc';
/** Refresh and purge scrubbed local metadata, preserving visible load failures. */
export function useAuditHistory(): {
  entries: AuditEntry[];
  failure: string;
  busy: boolean;
  refresh: () => Promise<void>;
  purge: () => Promise<boolean>;
} {
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [failure, setFailure] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    listAiAudit().then(
      (value) => {
        if (active) setEntries(value);
      },
      () => {
        if (active) setFailure('Audit history could not be loaded.');
      },
    );
    return () => {
      active = false;
    };
  }, []);
  const refresh = async (): Promise<void> => {
    setBusy(true);
    setFailure('');
    try {
      setEntries(await listAiAudit());
    } catch {
      setFailure('Audit history could not be loaded.');
    } finally {
      setBusy(false);
    }
  };
  const purge = async (): Promise<boolean> => {
    setBusy(true);
    setFailure('');
    try {
      await purgeAiAudit();
      setEntries([]);
      return true;
    } catch {
      setFailure('Audit history could not be purged.');
      return false;
    } finally {
      setBusy(false);
    }
  };
  return { entries, failure, busy, refresh, purge };
}
