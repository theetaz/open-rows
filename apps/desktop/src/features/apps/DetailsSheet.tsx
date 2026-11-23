// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Badge, Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { disconnectAgentApp, renewAgentApp, testAgentApp } from '@openrows/ipc';
import type { AppCard } from '@openrows/ipc';
import { ActivityList } from './ActivityList';
import { seedAgentActivity, useAgentActivity } from './activityStore';
import { AppGlyph } from './AppGlyph';
import { AppScope } from './AppScope';
import { renewable } from './appState';
import type { CheckState, Derived } from './appState';
import { appName, nowSeconds, reasonText } from './appNames';
import { CommandBlock } from './CommandBlock';
import styles from './Apps.module.css';
import { t } from './messages';

type Run = (work: () => Promise<string>) => void;

/** Test result or failure, shown under the actions until the next action. */
function useRun(): {
  busy: boolean;
  result: string;
  failure: string;
  run: Run;
} {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState('');
  const [failure, setFailure] = useState('');
  const run: Run = (work) => {
    setBusy(true);
    setResult('');
    setFailure('');
    work().then(
      (text) => {
        setBusy(false);
        setResult(text);
      },
      (error: unknown) => {
        setBusy(false);
        setFailure(error instanceof Error ? error.message : String(error));
      },
    );
  };
  return { busy, result, failure, run };
}

/** Install path and version, then the launch line the app's own tool reports. */
function Registration(props: {
  card: AppCard;
  check: CheckState | undefined;
}): React.JSX.Element {
  const line =
    props.check?.kind === 'done'
      ? (props.check.check.registration.line ?? props.check.check.expected)
      : null;
  return (
    <dl className={styles.facts}>
      <dt>{t('tool')}</dt>
      <dd>
        {props.card.install ? (
          <code>{props.card.install.path}</code>
        ) : (
          t('noTool')
        )}
      </dd>
      <dt>{t('registered')}</dt>
      <dd>{line ? <CommandBlock commands={[line]} /> : t('checking')}</dd>
      {props.card.link?.usage?.client && (
        <>
          <dt>{t('client')}</dt>
          <dd>{t('reportedAs', { client: props.card.link.usage.client })}</dd>
        </>
      )}
    </dl>
  );
}

/** Disconnect asks once more inline, naming what it runs. */
function DisconnectButton(props: {
  name: string;
  busy: boolean;
  onConfirm: () => void;
}): React.JSX.Element {
  const [asking, setAsking] = useState(false);
  if (!asking)
    return (
      <Button
        variant="destructive"
        disabled={props.busy}
        onClick={() => {
          setAsking(true);
        }}
      >
        {t('disconnect')}
      </Button>
    );
  return (
    <span className={styles.confirm}>
      <span title={t('disconnectExplain')}>
        {t('confirmDisconnect', { name: props.name })}
      </span>
      <Button
        variant="destructive"
        disabled={props.busy}
        onClick={props.onConfirm}
      >
        {t('disconnectNow')}
      </Button>
    </span>
  );
}

type ActionProps = {
  card: AppCard;
  derived: Derived;
  now: number;
  busy: boolean;
  run: Run;
  onClose: () => void;
  onChanged: () => void;
  onReconnect: () => void;
};

/** Disconnect on the leading side; Test or Reconnect, Renew and Done trailing. */
function DetailsActions(props: ActionProps): React.JSX.Element {
  const { card, derived, busy, run } = props;
  const app = card.app;
  return (
    <SheetActions
      leading={
        <DisconnectButton
          name={appName(app)}
          busy={busy}
          onConfirm={() => {
            run(async () => {
              const note = await disconnectAgentApp(app);
              props.onChanged();
              if (!note) props.onClose();
              return note ? t('removeNote', { note }) : '';
            });
          }}
        />
      }
    >
      {derived.status === 'attention' ? (
        <Button disabled={busy} onClick={props.onReconnect}>
          {t('reconnect')}
        </Button>
      ) : (
        <Button
          disabled={busy}
          onClick={() => {
            run(async () =>
              t('testPassed', { result: await testAgentApp(app) }),
            );
          }}
        >
          {busy ? t('testing') : t('test')}
        </Button>
      )}
      {(renewable(card, props.now) || derived.reasons.includes('expired')) && (
        <Button
          disabled={busy}
          onClick={() => {
            run(async () => {
              await renewAgentApp(app);
              props.onChanged();
              return '';
            });
          }}
        >
          {t('renew')}
        </Button>
      )}
      <Button variant="primary" onClick={props.onClose}>
        {t('done')}
      </Button>
    </SheetActions>
  );
}

/**
 * Details of a connected app: state and reasons, registered command, scope
 * with each connection's sharing level, its own activity, Test, Renew,
 * Reconnect and Disconnect.
 */
export function DetailsSheet(props: {
  card: AppCard;
  derived: Derived;
  check: CheckState | undefined;
  onClose: () => void;
  onChanged: () => void;
  onReconnect: () => void;
}): React.JSX.Element {
  const { card, derived } = props;
  const name = appName(card.app);
  const now = nowSeconds();
  const events = useAgentActivity().filter(
    (event) => event.token === card.link?.token,
  );
  const { busy, result, failure, run } = useRun();
  useEffect(() => {
    void seedAgentActivity();
  }, []);
  const detail = props.check?.kind === 'failed' ? props.check.message : '';
  return (
    <Sheet title={name} size="form" onClose={props.onClose}>
      <div className={styles.sheetBody}>
        <div className={styles.sheetLead}>
          <AppGlyph app={card.app} />
          <Badge tone={derived.status === 'connected' ? 'success' : 'warning'}>
            {derived.status === 'connected'
              ? t('connected')
              : t('needsAttention')}
          </Badge>
        </div>
        {derived.reasons.map((reason) => (
          <p key={reason} className={styles.reason}>
            {reasonText(reason, detail)}
          </p>
        ))}
        <Registration card={card} check={props.check} />
        {card.link?.scope && (
          <>
            <h4 className={styles.label}>{t('scope')}</h4>
            <AppScope scope={card.link.scope} />
          </>
        )}
        <h4 className={styles.label}>{t('recentActivity')}</h4>
        <ActivityList events={events} now={now} limit={5} />
        {result && <p role="status">{result}</p>}
        {failure && <p role="alert">{failure}</p>}
      </div>
      <DetailsActions
        card={card}
        derived={derived}
        now={now}
        busy={busy}
        run={run}
        onClose={props.onClose}
        onChanged={props.onChanged}
        onReconnect={props.onReconnect}
      />
    </Sheet>
  );
}
