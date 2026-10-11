// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { mcpStatus } from '@openrows/ipc';
import type { ServerStatus } from '@openrows/ipc';
/** Reconcile explicit MCP operations and retain one-time credentials only in this view. */
export function useMcpServer(): {
  status: ServerStatus | null;
  value: string;
  setValue: (value: string) => void;
  failure: string;
  busy: boolean;
  run: (work: () => Promise<void>) => Promise<void>;
} {
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [value, setValue] = useState('');
  const [failure, setFailure] = useState('');
  const [busy, setBusy] = useState(false);
  const run = async (work: () => Promise<void>): Promise<void> => {
    setBusy(true);
    setFailure('');
    try {
      await work();
      setStatus(await mcpStatus());
    } catch (error) {
      setFailure(
        error instanceof Error ? error.message : 'MCP operation failed.',
      );
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    let alive = true;
    mcpStatus().then(
      (next) => {
        if (alive) setStatus(next);
      },
      () => {
        if (alive) setFailure('MCP status unavailable.');
      },
    );
    return () => {
      alive = false;
    };
  }, []);
  return { status, value, setValue, failure, busy, run };
}
