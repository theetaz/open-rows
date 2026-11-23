// SPDX-License-Identifier: Apache-2.0
import { Badge } from '@openrows/ui';
import type { MonitorTable } from './monitorData';
import styles from './Monitor.module.css';
import parts from './MonitorParts.module.css';
import { t } from './messages/lazyDashboard';
type Row = MonitorTable['rows'][number];
type Graph = Map<string, Row[]>;

/** `Blocker`/`Blocked`, then the PID as a link to its session when visible. */
function NodeLabel(props: {
  pid: string;
  root: boolean;
  onSession: ((pid: string) => void) | undefined;
}): React.JSX.Element {
  const { pid, onSession } = props;
  return (
    <>
      <Badge tone={props.root ? 'danger' : 'warning'}>
        {props.root ? t('blocker') : t('blocked')}
      </Badge>
      {onSession ? (
        <button
          type="button"
          className={parts.linkButton}
          aria-label={t('sessionPid', { pid })}
          onClick={() => {
            onSession(pid);
          }}
        >
          {t('pidPid', { pid })}
        </button>
      ) : (
        t('pidPid', { pid })
      )}
    </>
  );
}

function branch(
  pid: string,
  graph: Graph,
  rows: Map<string, Row>,
  context: {
    seen: Set<string>;
    budget: { remaining: number };
    onSession?: ((pid: string) => void) | undefined;
  },
): React.JSX.Element | null {
  if (context.budget.remaining <= 0) return null;
  context.budget.remaining--;
  const { seen, budget } = context;
  const row = rows.get(pid);
  const cycle = seen.has(pid);
  const next = new Set([...seen, pid]);
  return (
    <li key={pid}>
      <NodeLabel
        pid={pid}
        root={!seen.size}
        onSession={row ? context.onSession : undefined}
      />
      {' · '}
      {row
        ? t('stateWaitQuery', {
            state: String(row.state ?? t('unknown')),
            wait: String(row.wait ?? t('noWait')),
            age_ms: String(row.age_ms ?? t('unknown')),
            transaction_ms: String(row.transaction_ms ?? t('unknown')),
          })
        : t('unknownBlockerOutside')}
      {row?.query && <pre>{String(row.query)}</pre>}
      {cycle ? (
        <> · {t('cycle')}</>
      ) : next.size > 16 ? (
        <> · {t('depthLimit')}</>
      ) : (
        <ul>
          {(graph.get(pid) ?? [])
            .slice(0, Math.max(0, budget.remaining))
            .map((child) =>
              branch(String(child.pid), graph, rows, {
                seen: next,
                budget,
                onSession: context.onSession,
              }),
            )}
        </ul>
      )}
    </li>
  );
}
/** Blocker → blocked tree, retaining all parents, unknown owners and bounded cycles. */
export function BlockingTree({
  table,
  onSession,
}: {
  table: MonitorTable | undefined;
  onSession?: ((pid: string) => void) | undefined;
}): React.JSX.Element {
  const rows = new Map(
    (table?.rows ?? []).map((row) => [String(row.pid), row]),
  );
  const graph: Graph = new Map();
  const waiting = new Set<string>();
  for (const row of table?.rows ?? []) {
    const blockers = String(row.blockers ?? '')
      .replace(/[{}]/g, '')
      .split(',')
      .filter(Boolean);
    if (blockers.length) waiting.add(String(row.pid));
    for (const blocker of blockers.slice(0, 100))
      graph.set(blocker, [...(graph.get(blocker) ?? []), row]);
  }
  const roots = [...graph.keys()].filter((pid) => !waiting.has(pid));
  if (!roots.length && graph.size) roots.push(...graph.keys());
  const budget = { remaining: 100 };
  return (
    <>
      <p className={styles.note}>{t('blockerBlockedQuery')}</p>
      {graph.size ? (
        <ul className={parts.tree} aria-label={t('blockingChains')}>
          {roots
            .slice(0, 100)
            .map((pid) =>
              branch(pid, graph, rows, { seen: new Set(), budget, onSession }),
            )}
        </ul>
      ) : (
        <p>{t('noBlockingEdges')}</p>
      )}
      {budget.remaining === 0 && <p>{t('graphDisplayLimit')}</p>}
    </>
  );
}
