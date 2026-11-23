// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import { checkAgentApp, listAgentApps } from '@openrows/ipc';
import type { AgentApp, AppsOverview } from '@openrows/ipc';
import type { CheckState } from './appState';
import { t } from './messages';

type Checks = Partial<Record<AgentApp, CheckState>>;

export type AgentApps = {
  overview: AppsOverview | null;
  checks: Checks;
  failure: string;
  loading: boolean;
  /** Detect again, then re-check every linked app with its own tool. */
  refresh: () => Promise<void>;
};

function message(error: unknown): string {
  return error instanceof Error ? error.message : t('appsUnavailable');
}

/**
 * The Apps page's data: detection runs when the page opens (a user action),
 * then each connected app's own `mcp get`/`list` runs in parallel so the
 * cards appear at once and their checks fill in.
 */
export function useAgentApps(): AgentApps {
  const [overview, setOverview] = useState<AppsOverview | null>(null);
  const [checks, setChecks] = useState<Checks>({});
  const [failure, setFailure] = useState('');
  const [loading, setLoading] = useState(true);
  const alive = useRef(true);
  const check = useCallback((app: AgentApp) => {
    setChecks((current) => ({ ...current, [app]: { kind: 'pending' } }));
    checkAgentApp(app).then(
      (result) => {
        if (alive.current)
          setChecks((current) => ({
            ...current,
            [app]: { kind: 'done', check: result },
          }));
      },
      (error: unknown) => {
        if (alive.current)
          setChecks((current) => ({
            ...current,
            [app]: { kind: 'failed', message: message(error) },
          }));
      },
    );
  }, []);
  const load = useCallback(
    () =>
      listAgentApps().then(
        (next) => {
          if (!alive.current) return;
          setOverview(next);
          setChecks({});
          setLoading(false);
          for (const card of next.apps)
            if (card.link && card.install) check(card.app);
        },
        (error: unknown) => {
          if (!alive.current) return;
          setFailure(message(error));
          setLoading(false);
        },
      ),
    [check],
  );
  const refresh = useCallback(async () => {
    setLoading(true);
    setFailure('');
    await load();
  }, [load]);
  useEffect(() => {
    alive.current = true;
    void load();
    return () => {
      alive.current = false;
    };
  }, [load]);
  return { overview, checks, failure, loading, refresh };
}
