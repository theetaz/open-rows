// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Switch, TextField } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import { validateMonitorIndex } from '@openrows/ipc';
import type { HypoPgReply, MonitorTarget, Suggestion } from '@openrows/ipc';
import styles from './Monitor.module.css';
import parts from './MonitorParts.module.css';
import { rich, t } from './messages/lazyInvestigation';

type Props = {
  target: MonitorTarget;
  card: Suggestion;
  querySql: string;
  onClose: () => void;
};

/** An explicit per-check opt-in; hypothetical indexes live only in a disposable session. */
export function HypoPgReview(props: Props): React.JSX.Element {
  const [sql, setSql] = useState(props.querySql);
  const [schema, setSchema] = useState('public');
  const [enabled, setEnabled] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  const [running, setRunning] = useState(false);
  const [reply, setReply] = useState<HypoPgReply | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = (confirmToken: string | null): void => {
    if (!props.card.statementId) return;
    setRunning(true);
    setError(null);
    validateMonitorIndex({
      target: props.target,
      statementId: props.card.statementId,
      suggestionId: props.card.id,
      sql,
      schema,
      enabled,
      confirmToken,
    }).then(
      (result) => {
        setReply(result);
        setRunning(false);
      },
      () => {
        setError(t('hypopgAbsentDenied'));
        setRunning(false);
      },
    );
  };
  const changed = (): void => {
    setReviewed(false);
    setReply(null);
  };
  return (
    <Sheet title={t('hypopgWhatIf')} onClose={props.onClose}>
      <div className={parts.review}>
        <p className={styles.note}>{t('optionalOffDefault')}</p>
        <ReviewFields
          sql={sql}
          schema={schema}
          enabled={enabled}
          reviewed={reviewed}
          running={running}
          onSql={(value) => {
            setSql(value);
            changed();
          }}
          onSchema={(value) => {
            setSchema(value);
            changed();
          }}
          onEnabled={setEnabled}
          onReviewed={setReviewed}
        />
        <div className={styles.actions}>
          <Button
            disabled={!enabled || !reviewed || running}
            onClick={() => {
              run(null);
            }}
          >
            {t('comparePlans')}
          </Button>
          <Button onClick={props.onClose}>{t('close')}</Button>
        </div>
        <ReviewResult running={running} error={error} reply={reply} run={run} />
      </div>
    </Sheet>
  );
}

function ReviewFields(props: {
  sql: string;
  schema: string;
  enabled: boolean;
  reviewed: boolean;
  running: boolean;
  onSql: (value: string) => void;
  onSchema: (value: string) => void;
  onEnabled: (value: boolean) => void;
  onReviewed: (value: boolean) => void;
}): React.JSX.Element {
  return (
    <>
      <textarea
        aria-label={t('reviewedWhatIf')}
        value={props.sql}
        disabled={props.running}
        onChange={(event) => {
          props.onSql(event.target.value);
        }}
      />
      <TextField
        aria-label={t('whatIfSchema')}
        value={props.schema}
        disabled={props.running}
        onChange={(event) => {
          props.onSchema(event.target.value);
        }}
      />
      <label>
        {t('statementBindingsReviewed')}{' '}
        <Switch
          aria-label={t('whatIfSql')}
          checked={props.reviewed}
          onCheckedChange={props.onReviewed}
        />
      </label>
      <label>
        {t('enableHypopgCheck')}{' '}
        <Switch
          aria-label={t('enableHypopg')}
          checked={props.enabled}
          onCheckedChange={props.onEnabled}
        />
      </label>
    </>
  );
}

function ReviewResult(props: {
  running: boolean;
  error: string | null;
  reply: HypoPgReply | null;
  run: (token: string | null) => void;
}): React.JSX.Element {
  return (
    <>
      {props.running && <p role="status">{t('comparingEstimatedPlans')}</p>}
      {props.error && <p role="alert">{props.error}</p>}
      {props.reply?.kind === 'confirmationRequired' && (
        <section role="alertdialog" aria-label={t('confirmWhatIf')}>
          <p>{t('safetyPolicyRequires')}</p>
          <Button
            onClick={() => {
              props.run(
                props.reply?.kind === 'confirmationRequired'
                  ? props.reply.token
                  : null,
              );
            }}
          >
            {t('confirmPlanRequest')}
          </Button>
        </section>
      )}
      {props.reply?.kind === 'estimate' && (
        <p role="status">
          {rich('estimatedTotalCost', {
            baselineCost: props.reply.estimate.baselineCost,
            hypotheticalCost: props.reply.estimate.hypotheticalCost,
          })}
        </p>
      )}
    </>
  );
}
