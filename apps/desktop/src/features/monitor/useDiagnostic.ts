// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { cancelQuery, explainMonitorQuery } from '@openrows/ipc';
import type { ExplainReply, MonitorExplain } from '@openrows/ipc';

/** Monitor diagnostics cancel on close and bind confirmation to the immutable reviewed request. */
export function useDiagnostic(onPlan: () => void): {
  reply: ExplainReply | null;
  running: boolean;
  error: string | null;
  run: (request: MonitorExplain) => void;
  confirm: () => void;
  clear: () => void;
} {
  const current = useRef<MonitorExplain | null>(null);
  const [reply, setReply] = useState<ExplainReply | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(
    () => () => {
      if (current.current) void cancelQuery(current.current.explainId);
      current.current = null;
    },
    [],
  );
  const run = (request: MonitorExplain): void => {
    current.current = request;
    setReply(null);
    setError(null);
    setRunning(true);
    explainMonitorQuery(request).then(
      (result) => {
        if (current.current?.explainId !== request.explainId) return;
        setReply(result);
        setRunning(false);
        if (result.kind === 'explained') onPlan();
      },
      () => {
        if (current.current?.explainId !== request.explainId) return;
        setError('Diagnostic failed; review the SQL, permissions and context.');
        setRunning(false);
      },
    );
  };
  return {
    reply,
    running,
    error,
    run,
    confirm: () => {
      if (reply?.kind === 'confirmationRequired' && current.current)
        run({
          ...current.current,
          explainId: crypto.randomUUID(),
          confirmToken: reply.token,
        });
    },
    clear: () => {
      if (current.current) void cancelQuery(current.current.explainId);
      current.current = null;
      setReply(null);
      setRunning(false);
      setError(null);
    },
  };
}
