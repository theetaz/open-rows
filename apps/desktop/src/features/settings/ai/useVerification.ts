// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { cancelAiVerification, verifyAiProvider } from '@openrows/ipc';
import type { Verification } from '@openrows/ipc';
/** Probe only after explicit action; cancel native work when its row leaves. */
export function useVerification(id: string): {
  result: Verification | null;
  busy: boolean;
  failure: string;
  verify: () => Promise<void>;
} {
  const [result, setResult] = useState<Verification | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState('');
  useEffect(
    () => () => {
      void cancelAiVerification(id).catch(() => undefined);
    },
    [id],
  );
  const verify = async (): Promise<void> => {
    setBusy(true);
    setFailure('');
    try {
      setResult(await verifyAiProvider(id));
    } catch {
      setFailure(
        'Verification unavailable. Check AI permission and saved settings.',
      );
    } finally {
      setBusy(false);
    }
  };
  return { result, busy, failure, verify };
}
