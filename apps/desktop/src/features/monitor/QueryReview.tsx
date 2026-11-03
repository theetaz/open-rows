// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button, Switch, TextField } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { MonitorTarget } from '@openrows/ipc';
import { useDiagnostic } from './useDiagnostic';
import styles from './Monitor.module.css';
import { t } from './messages/lazyInvestigation';
const ExplainView = lazy(() =>
  import('../explain/ExplainView').then((module) => ({
    default: module.ExplainView,
  })),
);

/** Review bindings and context before an isolated, safety-gated diagnostic. */
export function QueryReview(props: {
  sql: string;
  target: MonitorTarget;
  statementId: string;
  production: boolean;
  onPlan: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const [sql, setSql] = useState(props.sql);
  const [schema, setSchema] = useState('public');
  const [reviewed, setReviewed] = useState(false);
  const [analyze, setAnalyze] = useState(false);
  const diagnostic = useDiagnostic(props.onPlan);
  const run = (measured: boolean): void => {
    setAnalyze(measured);
    diagnostic.run({
      target: props.target,
      statementId: props.statementId,
      sql,
      schema,
      analyze: measured,
      explainId: crypto.randomUUID(),
      confirmToken: null,
    });
  };
  const changed = (): void => {
    setReviewed(false);
    diagnostic.clear();
  };
  return (
    <Sheet title={t('reviewSqlBefore')} onClose={props.onClose}>
      <div className={styles.review}>
        <p>
          {t('collectedLiteralsBindings', { database: props.target.database })}
        </p>
        <textarea
          aria-label={t('reviewedSql')}
          value={sql}
          disabled={diagnostic.running}
          onChange={(event) => {
            setSql(event.target.value);
            changed();
          }}
        />
        <TextField
          aria-label={t('diagnosticSchema')}
          value={schema}
          disabled={diagnostic.running}
          onChange={(event) => {
            setSchema(event.target.value);
            changed();
          }}
        />
        <label>
          {t('statementBindingsReviewed')}{' '}
          <Switch
            aria-label={t('statementReviewed')}
            checked={reviewed}
            onCheckedChange={setReviewed}
          />
        </label>
        <DiagnosticActions
          disabled={!reviewed || diagnostic.running}
          run={run}
        />
        <DiagnosticResult
          diagnostic={diagnostic}
          analyze={analyze}
          production={props.production}
        />
        <Button onClick={props.onClose}>{t('close')}</Button>
      </div>
    </Sheet>
  );
}

function DiagnosticResult({
  diagnostic,
  analyze,
  production,
}: {
  diagnostic: ReturnType<typeof useDiagnostic>;
  analyze: boolean;
  production: boolean;
}): React.JSX.Element {
  return (
    <>
      {diagnostic.reply?.kind === 'confirmationRequired' && (
        <section
          role="alertdialog"
          aria-label={t('confirmDiagnosticExecution')}
        >
          <p>
            {analyze
              ? t('explainAnalyseExecutes')
              : t('safetyPolicyRequiresConfirmation')}
          </p>
          {production && <p>{t('productionWarningMeasured')}</p>}
          <Button onClick={diagnostic.clear}>{t('cancelDiagnostic')}</Button>
          <Button onClick={diagnostic.confirm}>
            {analyze ? t('confirmMeasuredExecution') : t('confirmPlanRequest')}
          </Button>
        </section>
      )}
      {diagnostic.running && (
        <p role="status">
          {t('requestingDiagnostic')}{' '}
          <Button onClick={diagnostic.clear}>{t('cancelDiagnostic')}</Button>
        </p>
      )}
      {diagnostic.reply?.kind === 'blocked' && (
        <p role="alert">{diagnostic.reply.message}</p>
      )}
      {diagnostic.error && <p role="alert">{diagnostic.error}</p>}
      {diagnostic.reply?.kind === 'explained' && (
        <Suspense fallback={null}>
          <ExplainView plan={diagnostic.reply.plan} />
        </Suspense>
      )}
    </>
  );
}

function DiagnosticActions(props: {
  disabled: boolean;
  run: (measured: boolean) => void;
}): React.JSX.Element {
  return (
    <div className={styles.actions}>
      <Button
        disabled={props.disabled}
        onClick={() => {
          props.run(false);
        }}
      >
        {t('explainPlanOnly')}
      </Button>
      <Button
        disabled={props.disabled}
        onClick={() => {
          props.run(true);
        }}
      >
        {t('explainAnalyseRuns')}
      </Button>
    </div>
  );
}
