// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button, Switch, TextField } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { MonitorTarget } from '@openrows/ipc';
import { useDiagnostic } from './useDiagnostic';
import styles from './Monitor.module.css';
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
    <Sheet title="Review SQL before Explain" onClose={props.onClose}>
      <div className={styles.review}>
        <p>
          Collected literals and bindings are redacted. Supply a complete
          statement. Diagnostics use a separate session on{' '}
          {props.target.database}, with search path pg_catalog and the reviewed
          schema; profile startup SQL and your open transaction are excluded.
        </p>
        <textarea
          aria-label="Reviewed SQL"
          value={sql}
          disabled={diagnostic.running}
          onChange={(event) => {
            setSql(event.target.value);
            changed();
          }}
        />
        <TextField
          aria-label="Diagnostic schema"
          value={schema}
          disabled={diagnostic.running}
          onChange={(event) => {
            setSchema(event.target.value);
            changed();
          }}
        />
        <label>
          Statement and bindings reviewed{' '}
          <Switch
            aria-label="Statement reviewed"
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
        <Button onClick={props.onClose}>Close</Button>
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
        <section role="alertdialog" aria-label="Confirm diagnostic execution">
          <p>
            {analyze
              ? 'Explain analyse executes this reviewed statement. Rollback cannot undo every effect, including sequences and external functions.'
              : 'Your safety policy requires confirmation before requesting this plan.'}
          </p>
          {production && (
            <p>
              Production warning: measured execution can add load, take locks
              and affect production data. Review workload impact before
              confirming.
            </p>
          )}
          <Button onClick={diagnostic.clear}>Cancel diagnostic</Button>
          <Button onClick={diagnostic.confirm}>
            {analyze ? 'Confirm measured execution' : 'Confirm plan request'}
          </Button>
        </section>
      )}
      {diagnostic.running && (
        <p role="status">
          Requesting diagnostic…{' '}
          <Button onClick={diagnostic.clear}>Cancel diagnostic</Button>
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
        Explain plan only
      </Button>
      <Button
        disabled={props.disabled}
        onClick={() => {
          props.run(true);
        }}
      >
        Explain analyse (runs SQL)
      </Button>
    </div>
  );
}
