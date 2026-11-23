// SPDX-License-Identifier: Apache-2.0
import { memo } from 'react';
import { Badge, Button } from '@openrows/ui';
import type { AppCard as Card } from '@openrows/ipc';
import { AppGlyph } from './AppGlyph';
import type { CheckState, Derived } from './appState';
import { appBlurb, appName, reasonText, relativeTime } from './appNames';
import styles from './Apps.module.css';
import { t, tn } from './messages';

type Tone = 'success' | 'warning' | 'neutral' | 'info';

function pill(card: Card, derived: Derived): { tone: Tone; label: string } {
  switch (derived.status) {
    case 'connected':
      return { tone: 'success', label: t('connected') };
    case 'attention':
      return { tone: 'warning', label: t('needsAttention') };
    case 'checking':
      return { tone: 'info', label: t('checking') };
    case 'installed':
      return {
        tone: 'neutral',
        label: card.install?.version
          ? t('installedVersion', { version: card.install.version })
          : t('installed'),
      };
    case 'notInstalled':
      return { tone: 'neutral', label: t('notInstalled') };
  }
}

/** Last use and call count for a linked app, or the first attention reason. */
function Meta(props: {
  card: Card;
  derived: Derived;
  check: CheckState | undefined;
  seen: number | null;
  now: number;
}): React.JSX.Element {
  const { card, derived } = props;
  const [reason] = derived.reasons;
  if (reason) {
    const detail = props.check?.kind === 'failed' ? props.check.message : '';
    return <p className={styles.reason}>{reasonText(reason, detail)}</p>;
  }
  if (!card.link)
    return (
      <p className={styles.meta}>
        {card.install ? appBlurb(card.app) : t('installHint')}
      </p>
    );
  const calls = card.link.usage?.calls ?? 0;
  return (
    <p className={styles.meta}>
      {props.seen === null
        ? t('neverUsed')
        : t('lastUsed', { when: relativeTime(props.seen, props.now) })}
      {' · '}
      {tn('calls', calls, { count: String(calls) })}
    </p>
  );
}

/** One app in the directory grid: glyph, name, state pill and its action. */
export const AppCard = memo(function AppCard(props: {
  card: Card;
  derived: Derived;
  check: CheckState | undefined;
  /** Latest use, from usage or a live event. */
  seen: number | null;
  now: number;
  busy: boolean;
  onConnect: (card: Card) => void;
  onDetails: (card: Card) => void;
  onRefresh: () => void;
}): React.JSX.Element {
  const { card, derived } = props;
  const name = appName(card.app);
  const badge = pill(card, derived);
  return (
    <article
      className={styles.card}
      aria-label={name}
      data-status={derived.status}
    >
      <div className={styles.cardHead}>
        <AppGlyph app={card.app} />
        <div className={styles.cardTitle}>
          <h4>{name}</h4>
          <Badge tone={badge.tone}>{badge.label}</Badge>
        </div>
      </div>
      <Meta {...props} />
      <div className={styles.cardActions}>
        <CardAction {...props} name={name} />
      </div>
    </article>
  );
});

function CardAction(props: {
  card: Card;
  derived: Derived;
  name: string;
  busy: boolean;
  onConnect: (card: Card) => void;
  onDetails: (card: Card) => void;
  onRefresh: () => void;
}): React.JSX.Element {
  const { card, derived } = props;
  if (derived.status === 'notInstalled')
    return (
      <Button size="sm" disabled={props.busy} onClick={props.onRefresh}>
        {t('checkAgain')}
      </Button>
    );
  if (derived.status === 'installed')
    return (
      <Button
        size="sm"
        variant="primary"
        aria-label={t('connectName', { name: props.name })}
        disabled={props.busy}
        onClick={() => {
          props.onConnect(card);
        }}
      >
        {t('connect')}
      </Button>
    );
  return (
    <Button
      size="sm"
      aria-label={t('detailsName', { name: props.name })}
      onClick={() => {
        props.onDetails(card);
      }}
    >
      {derived.status === 'attention' ? t('fix') : t('details')}
    </Button>
  );
}
