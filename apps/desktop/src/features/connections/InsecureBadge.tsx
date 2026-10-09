// SPDX-License-Identifier: Apache-2.0
import { Badge, Icon } from '@openrows/ui';
import type { InsecureReasonDto } from '@openrows/ipc';
import { insecureDetail } from './insecure';
import styles from './InsecureBadge.module.css';

/**
 * "Not secure" flag for a connection (docs/12 §4.1): skip-verify or
 * disabled TLS towards a remote host, or an SSH host key accepted once.
 * `compact` shows only the open-lock glyph inside a rail row, whose
 * tooltip states the reasons; the full badge names them for assistive tech.
 */
export function InsecureBadge({
  reasons,
  compact = false,
}: {
  reasons: readonly InsecureReasonDto[];
  compact?: boolean;
}): React.JSX.Element | null {
  if (reasons.length === 0) return null;
  const detail = insecureDetail(reasons);
  if (compact)
    return (
      <span className={styles.mark} aria-hidden="true" title={detail}>
        <Icon name="unlock" />
      </span>
    );
  return (
    <Badge tone="warning" className={styles.badge} title={detail}>
      <Icon name="unlock" />
      Not secure
      <span className={styles.hidden}>{`: ${detail}`}</span>
    </Badge>
  );
}
