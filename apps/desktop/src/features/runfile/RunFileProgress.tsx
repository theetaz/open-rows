// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { cancelQuery } from '@openrows/ipc';
import { showRunFile, useRun } from './runFileStore';
import type { FileRunState, RunError } from './runFileStore';
import { sizeText } from './runFileJob';
import { elapsedText, useElapsed } from './elapsed';
import styles from './RunFile.module.css';
import { t, tn } from './lazyMessages';

/** A started file run: live progress, then the summary and every error. */
export function RunFileProgress({
  id,
}: {
  id: string;
}): React.JSX.Element | null {
  const run = useRun(id);
  const running = !!run && !run.finished && !run.failure;
  const elapsed = useElapsed(run?.startedAt ?? 0, running);
  if (!run) return null;
  const close = (): void => {
    showRunFile(null);
  };
  const runId = run.runId;
  return (
    <Sheet title={t('runName', { name: run.file.name })} onClose={close}>
      {running ? (
        <Running run={run} elapsed={elapsed} />
      ) : (
        <Summary run={run} />
      )}
      <ErrorList errors={run.errors} total={run.errorCount} />
      <SheetActions>
        {running && (
          <Button
            variant="destructive"
            disabled={!runId}
            onClick={() => {
              if (runId) void cancelQuery(runId);
            }}
          >
            {t('cancelRun')}
          </Button>
        )}
        <Button variant="primary" onClick={close}>
          {running ? t('runBackground') : t('done')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function Running({
  run,
  elapsed,
}: {
  run: FileRunState;
  elapsed: number;
}): React.JSX.Element {
  return (
    <div className={styles.status} role="status">
      <p className={styles.counts}>
        <span>
          {t('statementsTotalStatements', {
            statements: run.statements.toLocaleString(),
            total: run.total.toLocaleString(),
          })}
        </span>
        <span>
          {t('bytesCount', {
            bytes: sizeText(run.bytes),
            count: sizeText(run.size),
          })}
        </span>
        <span>{elapsedText(elapsed)}</span>
      </p>
      <progress
        aria-label={t('runProgress')}
        max={Math.max(run.size, 1)}
        value={run.bytes}
      />
      {run.current && (
        <code className={styles.current} title={run.current}>
          {run.current}
        </code>
      )}
    </div>
  );
}

function outcomeText(run: FileRunState): string {
  if (run.failure) return run.failure;
  const finished = run.finished;
  if (!finished) return '';
  const counts = {
    statements: (finished.statements ?? 0).toLocaleString(),
    total: run.total.toLocaleString(),
    elapsed: elapsedText(finished.elapsedMs ?? 0),
  };
  if (finished.outcome === 'completed')
    return finished.rowsAffected
      ? t('doneWithRows', {
          ...counts,
          rows: finished.rowsAffected.toLocaleString(),
        })
      : t('doneRan', counts);
  const rolled = finished.rolledBack ? [t('transactionWasRolled')] : [];
  if (finished.outcome === 'cancelled')
    return [t('cancelledRan', counts), ...rolled].join(' ');
  const reason = finished.message
    ? [t('reasonSentence', { reason: finished.message })]
    : [];
  return [t('stoppedWithErrors', counts), ...reason, ...rolled].join(' ');
}

function Summary({ run }: { run: FileRunState }): React.JSX.Element {
  const failed = !!run.failure || run.finished?.outcome === 'failed';
  return (
    <p
      className={failed ? styles.error : styles.done}
      role={failed ? 'alert' : 'status'}
    >
      {outcomeText(run)}
    </p>
  );
}

function ErrorList({
  errors,
  total,
}: {
  errors: readonly RunError[];
  total: number;
}): React.JSX.Element | null {
  if (!errors.length) return null;
  return (
    <section className={styles.errors} aria-label={t('errors')}>
      <h3>
        {total > errors.length
          ? tn('countErrorsFirst', total, {
              count: total.toLocaleString(),
              count2: errors.length.toLocaleString(),
            })
          : tn('countErrors', total, { count: total.toLocaleString() })}
      </h3>
      <ol>
        {errors.map((error) => (
          <li key={error.statement}>
            <span className={styles.line}>
              {t('lineLine', { line: error.line })}
            </span>
            <code title={error.preview}>{error.preview}</code>
            <span className={styles.message}>{error.message}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}
