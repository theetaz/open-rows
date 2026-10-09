// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { cancelQuery } from '@openrows/ipc';
import { showRunFile, useRun } from './runFileStore';
import type { FileRunState, RunError } from './runFileStore';
import { sizeText } from './runFileJob';
import { elapsedText, useElapsed } from './elapsed';
import styles from './RunFile.module.css';

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
    <Sheet title={`Run ${run.file.name}`} onClose={close}>
      {running ? (
        <Running run={run} elapsed={elapsed} />
      ) : (
        <Summary run={run} />
      )}
      <ErrorList errors={run.errors} total={run.errorCount} />
      <SheetActions>
        {running && (
          <Button
            disabled={!runId}
            onClick={() => {
              if (runId) void cancelQuery(runId);
            }}
          >
            Cancel Run
          </Button>
        )}
        <Button variant="primary" onClick={close}>
          {running ? 'Run in Background' : 'Done'}
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
      <progress
        aria-label="Run progress"
        max={Math.max(run.size, 1)}
        value={run.bytes}
      />
      <p className={styles.counts}>
        <span>
          {run.statements.toLocaleString()} of {run.total.toLocaleString()}{' '}
          statements
        </span>
        <span>
          {sizeText(run.bytes)} of {sizeText(run.size)}
        </span>
        <span>{elapsedText(elapsed)}</span>
      </p>
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
  const ran = `${(finished.statements ?? 0).toLocaleString()} of ${run.total.toLocaleString()} statements ran`;
  const rows = finished.rowsAffected
    ? `, ${finished.rowsAffected.toLocaleString()} rows affected`
    : '';
  const rolled = finished.rolledBack ? ' The transaction was rolled back.' : '';
  const took = ` in ${elapsedText(finished.elapsedMs ?? 0)}`;
  if (finished.outcome === 'completed') return `Done: ${ran}${rows}${took}.`;
  if (finished.outcome === 'cancelled')
    return `Cancelled: ${ran}${took}.${rolled}`;
  const reason = finished.message ? ` ${finished.message}.` : '';
  return `Stopped with errors: ${ran}${took}.${reason}${rolled}`;
}

function Summary({ run }: { run: FileRunState }): React.JSX.Element {
  const failed = !!run.failure || run.finished?.outcome === 'failed';
  return (
    <p
      className={failed ? styles.error : styles.summary}
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
    <section className={styles.errors} aria-label="Errors">
      <h3>
        {total.toLocaleString()} {total === 1 ? 'error' : 'errors'}
        {total > errors.length &&
          ` (first ${errors.length.toLocaleString()} shown)`}
      </h3>
      <ol>
        {errors.map((error) => (
          <li key={error.statement}>
            <span className={styles.line}>Line {error.line}</span>
            <code title={error.preview}>{error.preview}</code>
            <span className={styles.message}>{error.message}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}
