// SPDX-License-Identifier: Apache-2.0
import type { Connections } from './useConnections';
import styles from './ConnectionHeader.module.css';

/**
 * Connection failures and action errors above the object tree. Identity,
 * status and actions live in the workspace sidebar row and the context bar.
 */
export function ConnectionHeader({
  connections,
}: {
  connections: Connections;
}): React.JSX.Element | null {
  const { active, problem } = connections;
  const status = active ? connections.statusOf(active.id) : undefined;
  const failure = status?.state === 'failed' ? status.message : null;
  if (!failure && !problem) return null;
  return (
    <div className={styles.header}>
      {failure && (
        <p className={styles.problem} role="alert">
          {failure}
        </p>
      )}
      {problem && <p className={styles.problem}>{problem}</p>}
    </div>
  );
}
