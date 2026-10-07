// SPDX-License-Identifier: Apache-2.0
import type { CSSProperties } from 'react';
import { Badge, EnvBadge } from '@openrows/ui';
import type { EnvDto } from '@openrows/ipc';
import styles from './Connections.module.css';

/** Standard environments use theme tokens; custom ones show their own colour and label. */
export function EnvironmentBadge({ env }: { env: EnvDto }): React.JSX.Element {
  if (env.kind !== 'custom') return <EnvBadge environment={env.kind} />;
  return (
    <Badge tone="neutral">
      <span
        aria-hidden="true"
        className={styles.customDot}
        style={customColour(env)}
      />
      {env.label ?? 'custom'}
    </Badge>
  );
}

/** Inline custom property carrying a user-chosen colour (never a theme colour). */
export function customColour(env: EnvDto): CSSProperties | undefined {
  return env.kind === 'custom'
    ? ({ '--env-color': env.color } as CSSProperties)
    : undefined;
}
