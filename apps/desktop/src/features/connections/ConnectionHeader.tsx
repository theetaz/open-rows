// SPDX-License-Identifier: Apache-2.0
import type { Connections } from './useConnections';
import { POLICY_LABELS } from '../security/policies';
import styles from './ConnectionHeader.module.css';

/**
 * Connection/database identity and failures above the object tree.
 * Status and actions also live in the workspace sidebar and context bar.
 */
export function ConnectionHeader({
  connections,
  database,
}: {
  connections: Connections;
  database?: string | undefined;
}): React.JSX.Element | null {
  const { active, problem } = connections;
  const status = active ? connections.statusOf(active.id) : undefined;
  const failure = status?.state === 'failed' ? status.message : null;
  if (!active && !failure && !problem) return null;
  return (
    <div className={styles.header}>
      {active && (
        <p aria-label="Sidebar context">
          {active.name} · {database || active.database || 'No database'} ·{' '}
          {POLICY_LABELS[active.safety]}
        </p>
      )}
      {failure && (
        <p className={styles.problem} role="alert">
          {failure}
        </p>
      )}
      {problem && <p className={styles.problem}>{problem}</p>}
    </div>
  );
}
