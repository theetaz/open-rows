// SPDX-License-Identifier: Apache-2.0
import { Badge, EnvBadge, StatusDot } from '@openrows/ui';
import styles from './Gallery.module.css';

const statuses = [
  { tone: 'warning', label: 'Pending' },
  { tone: 'success', label: 'Shipped' },
  { tone: 'info', label: 'Paid' },
  { tone: 'neutral', label: 'Cancelled' },
  { tone: 'danger', label: 'Failed' },
] as const;
/** Text-labelled semantic pills and environment identities. */
export function IndicatorGallery(): React.JSX.Element {
  return (
    <section
      id="indicators"
      aria-labelledby="indicators-heading"
      className={styles.section}
    >
      <h2 id="indicators-heading">Environment & status</h2>
      <div className={styles.row}>
        {(['local', 'dev', 'staging', 'prod'] as const).map((environment) => (
          <EnvBadge key={environment} environment={environment} />
        ))}
      </div>
      <div className={styles.row}>
        {statuses.map(({ tone, label }) => (
          <Badge key={tone} tone={tone}>
            {label}
          </Badge>
        ))}
      </div>
      <div className={styles.row}>
        {statuses.map(({ tone, label }) => (
          <StatusDot key={tone} tone={tone} label={label} />
        ))}
      </div>
    </section>
  );
}
