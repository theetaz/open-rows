// SPDX-License-Identifier: Apache-2.0
import type { MonitorTable } from './monitorData';
import styles from './Monitor.module.css';
type Row = MonitorTable['rows'][number];
type Graph = Map<string, Row[]>;

function branch(
  pid: string,
  graph: Graph,
  rows: Map<string, Row>,
  context: { seen: Set<string>; budget: { remaining: number } },
): React.JSX.Element | null {
  if (context.budget.remaining <= 0) return null;
  context.budget.remaining--;
  const { seen, budget } = context;
  const row = rows.get(pid);
  const cycle = seen.has(pid);
  const next = new Set([...seen, pid]);
  return (
    <li key={pid}>
      PID {pid}
      {row
        ? ` · ${String(row.state ?? 'Unknown')} · ${String(row.wait ?? 'No wait')} · ${String(row.age_ms ?? 'Unknown')} ms · transaction ${String(row.transaction_ms ?? 'Unknown')} ms`
        : ' · Unknown blocker (outside visibility)'}
      {cycle ? (
        ' · Cycle'
      ) : next.size > 16 ? (
        ' · Depth limit'
      ) : (
        <ul>
          {(graph.get(pid) ?? [])
            .slice(0, Math.max(0, budget.remaining))
            .map((child) =>
              branch(String(child.pid), graph, rows, { seen: next, budget }),
            )}
        </ul>
      )}
    </li>
  );
}
/** Blocker → blocked tree, retaining all parents, unknown owners and bounded cycles. */
export function BlockingTree({
  table,
}: {
  table: MonitorTable | undefined;
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
      <p className={styles.note}>
        Blocker → blocked. Unseen owners, cycles and capped chains remain
        explicit.
      </p>
      {graph.size ? (
        <ul className={styles.tree} aria-label="Blocking chains">
          {roots
            .slice(0, 100)
            .map((pid) =>
              branch(pid, graph, rows, { seen: new Set(), budget }),
            )}
        </ul>
      ) : (
        <p>No blocking edges in this sample.</p>
      )}
      {budget.remaining === 0 && <p>Graph display limit (100 nodes)</p>}
    </>
  );
}
