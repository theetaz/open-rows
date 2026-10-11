// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { getAiSharing, saveAiSharing } from '@openrows/ipc';
import type { SharingPolicy } from '@openrows/ipc';
/** Load/save a complete native policy with fresh production confirmation for each edit. */
export function useSharingPolicy(id: string): {
  policy: Required<SharingPolicy> | null;
  dirty: boolean;
  busy: boolean;
  failure: string;
  saved: boolean;
  change: (policy: Required<SharingPolicy>) => void;
  save: () => Promise<void>;
} {
  const [policy, setPolicy] = useState<Required<SharingPolicy> | null>(null);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState('');
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    let active = true;
    getAiSharing(id).then(
      (value) => {
        if (active) setPolicy({ ...value, productionConfirmed: false });
      },
      () => {
        if (active) setFailure('Sharing settings could not be loaded.');
      },
    );
    return () => {
      active = false;
    };
  }, [id]);
  const change = (value: Required<SharingPolicy>): void => {
    setPolicy(value);
    setDirty(true);
    setSaved(false);
  };
  const save = async (): Promise<void> => {
    if (!policy) return;
    setBusy(true);
    setFailure('');
    setSaved(false);
    try {
      await saveAiSharing(id, policy);
      setDirty(false);
      setSaved(true);
    } catch (error) {
      setFailure(
        error instanceof Error
          ? error.message
          : 'Sharing policy could not be saved.',
      );
    } finally {
      setBusy(false);
    }
  };
  return { policy, dirty, busy, failure, saved, change, save };
}
