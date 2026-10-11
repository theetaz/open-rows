// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { onMcpProposals, takeMcpProposals } from '@openrows/ipc';
import { useQueryTabs } from '../query';
import type { QueryTabs } from '../query';
/** Model proposals only create editor tabs; the user's existing Run path retains SafetyPlan. */
export function useMcpProposals(tabs: QueryTabs, enabled: boolean): void {
  const latest = useRef(tabs);
  useEffect(() => {
    latest.current = tabs;
  }, [tabs]);
  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    let stop: (() => void) | undefined;
    const drain = async (): Promise<void> => {
      try {
        const proposals = await takeMcpProposals();
        if (alive)
          for (const proposal of proposals)
            latest.current.open(
              {
                connectionId: proposal.connection,
                database: proposal.database,
              },
              {
                title: 'SQL proposal — review before running',
                sql: proposal.sql,
              },
            );
      } catch {
        /* The native queue remains intact while the app is locked. */
      }
    };
    onMcpProposals(() => {
      void drain();
    }).then(
      (unlisten) => {
        if (!alive) {
          unlisten();
          return;
        }
        stop = unlisten;
        void drain();
      },
      () => undefined,
    );
    return () => {
      alive = false;
      stop?.();
    };
  }, [enabled]);
}

/** Keep proposal subscriptions attached to the workspace's ordinary query tabs. */
export function useMcpQueryTabs(enabled: boolean): QueryTabs {
  const tabs = useQueryTabs();
  useMcpProposals(tabs, enabled);
  return tabs;
}
