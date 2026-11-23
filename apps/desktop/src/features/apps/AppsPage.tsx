// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { Button, Icon } from '@openrows/ui';
import type { AgentApp, AppCard as Card } from '@openrows/ipc';
import { aiSettings } from '../settings/ai/aiSettings';
import { saveAiSettings } from '../settings/ai/aiActions';
import { ActivityList } from './ActivityList';
import { seedAgentActivity, useAgentActivity } from './activityStore';
import { AppCard } from './AppCard';
import { deriveStatus } from './appState';
import { nowSeconds } from './appNames';
import { ConnectSheet } from './ConnectSheet';
import { DetailsSheet } from './DetailsSheet';
import { useAgentApps } from './useAgentApps';
import type { AgentApps } from './useAgentApps';
import styles from './Apps.module.css';
import { t } from './messages';

type Open = { kind: 'connect' | 'details'; app: AgentApp } | null;

/** Server port, AI switch and bridge state above the grid. */
function Status(props: { apps: AgentApps }): React.JSX.Element {
  const ai = aiSettings.useValue();
  const overview = props.apps.overview;
  return (
    <>
      <p className={styles.status} role="status">
        <span className={styles.dot} data-on={overview?.port != null} />
        {overview?.port != null
          ? t('serverOn', { port: String(overview.port) })
          : t('serverOff')}
      </p>
      {!ai.enabled && (
        <div className={styles.callout}>
          <p>{t('aiOff')}</p>
          <Button
            size="sm"
            onClick={() => {
              void saveAiSettings({ ...ai, enabled: true }).then(
                props.apps.refresh,
              );
            }}
          >
            {t('turnOnAi')}
          </Button>
        </div>
      )}
      {overview?.bridgeProblem && (
        <p className={styles.reason}>
          {t('bridgeProblem', { problem: overview.bridgeProblem })}
        </p>
      )}
      {props.apps.failure && <p role="alert">{props.apps.failure}</p>}
    </>
  );
}

/** Latest use: the persisted counter or a newer live event for the same token. */
function lastSeen(
  card: Card,
  events: ReturnType<typeof useAgentActivity>,
): number | null {
  const token = card.link?.token;
  const live = events.find((event) => event.token === token)?.at ?? null;
  const stored = card.link?.usage?.seen ?? null;
  return live !== null && (stored === null || live > stored) ? live : stored;
}

/** The directory grid; cards only re-render when their own inputs change. */
function Grid(props: {
  apps: AgentApps;
  now: number;
  open: (next: Open) => void;
}): React.JSX.Element {
  const { overview, checks, loading, refresh } = props.apps;
  const { open } = props;
  const events = useAgentActivity();
  const port = overview?.port ?? null;
  const onConnect = useCallback(
    (card: Card) => {
      open({ kind: 'connect', app: card.app });
    },
    [open],
  );
  const onDetails = useCallback(
    (card: Card) => {
      open({ kind: 'details', app: card.app });
    },
    [open],
  );
  const onRefresh = useCallback(() => {
    void refresh();
  }, [refresh]);
  if (!overview) return <p role="status">{t('lookingForApps')}</p>;
  return (
    <div className={styles.grid} role="list" aria-label={t('appCards')}>
      {overview.apps.map((card) => (
        <div role="listitem" key={card.app} className={styles.cell}>
          <AppCard
            card={card}
            derived={deriveStatus(card, checks[card.app], port, props.now)}
            check={checks[card.app]}
            seen={lastSeen(card, events)}
            now={props.now}
            busy={loading}
            onConnect={onConnect}
            onDetails={onDetails}
            onRefresh={onRefresh}
          />
        </div>
      ))}
    </div>
  );
}

/**
 * Settings → AI → Apps: connect the agent CLIs the owner already uses to the
 * built-in MCP server in one confirmed step, and watch what they do.
 */
export default function AppsPage(props: {
  onProviders: () => void;
}): React.JSX.Element {
  const apps = useAgentApps();
  const events = useAgentActivity();
  const [open, setOpen] = useState<Open>(null);
  const now = nowSeconds();
  useEffect(() => {
    void seedAgentActivity();
  }, []);
  const card = apps.overview?.apps.find((item) => item.app === open?.app);
  const close = (): void => {
    setOpen(null);
  };
  return (
    <div role="region" aria-label={t('apps')} className={styles.page}>
      <p className={styles.intro}>{t('appsIntro')}</p>
      <Status apps={apps} />
      <Grid apps={apps} now={now} open={setOpen} />
      <div>
        <h3>{t('recentActivity')}</h3>
        <ActivityList events={events} now={now} limit={8} />
      </div>
      <Button
        size="sm"
        variant="ghost"
        className={styles.link}
        onClick={props.onProviders}
      >
        <Icon name="settings" />
        {t('advanced')}
      </Button>
      {open?.kind === 'connect' && (
        <ConnectSheet
          app={open.app}
          onClose={close}
          onProviders={() => {
            close();
            props.onProviders();
          }}
          onConnected={() => {
            close();
            void apps.refresh();
          }}
        />
      )}
      {open?.kind === 'details' && card && (
        <DetailsSheet
          card={card}
          derived={deriveStatus(
            card,
            apps.checks[card.app],
            apps.overview?.port ?? null,
            now,
          )}
          check={apps.checks[card.app]}
          onClose={close}
          onChanged={() => void apps.refresh()}
          onReconnect={() => {
            setOpen({ kind: 'connect', app: card.app });
          }}
        />
      )}
    </div>
  );
}
