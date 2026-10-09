// SPDX-License-Identifier: Apache-2.0
import type { ImportSummaryDto, RowErrorDto } from '@openrows/ipc';
import { rowCount } from './job';
import type { ImportJob, ImportProgress } from './job';
import styles from './Import.module.css';

const count = (value: number | null): string => (value ?? 0).toLocaleString();

/** Progress while running, then the summary and the row error report. */
export function ImportStatus(props: {
  job: ImportJob;
  table: string;
}): React.JSX.Element | null {
  const { job } = props;
  switch (job.status) {
    case 'idle':
      return null;
    case 'starting':
      return (
        <p className={styles.status} role="status">
          {job.mode === 'check' ? 'Checking the file…' : 'Starting the import…'}
        </p>
      );
    case 'running':
      return <Running mode={job.mode} progress={job.progress} />;
    case 'finished':
      return job.mode === 'check' ? (
        <CheckSummary summary={job.summary} />
      ) : (
        <Summary
          text={`Imported ${rowCount(job.summary.inserted)} into ${props.table}.`}
          summary={job.summary}
        />
      );
    case 'cancelled':
      return (
        <Summary
          text={
            job.mode === 'check'
              ? 'Check cancelled.'
              : 'Import cancelled. Uncommitted rows were rolled back.'
          }
          summary={job.summary}
        />
      );
    case 'failed':
      return <Summary text={job.message} summary={job.summary} alert />;
  }
}

function Running(props: {
  mode: 'check' | 'import';
  progress: ImportProgress;
}): React.JSX.Element {
  const { progress } = props;
  return (
    <div className={styles.status} role="status">
      <p>{props.mode === 'check' ? 'Checking…' : 'Importing…'}</p>
      <progress
        aria-label={
          props.mode === 'check' ? 'Check progress' : 'Import progress'
        }
        max={1}
        value={progress.fraction}
      />
      <p className={styles.note}>
        {Math.round(progress.fraction * 100)}% · {rowCount(progress.rows)} read
        · {count(progress.inserted)} written · {count(progress.failed)} failed ·{' '}
        {count(progress.rowsPerSecond)} rows/s
      </p>
    </div>
  );
}

function CheckSummary({
  summary,
}: {
  summary: ImportSummaryDto;
}): React.JSX.Element {
  const conflicts =
    summary.conflicts === null
      ? 'Conflicts cannot be checked without key columns.'
      : `${rowCount(summary.conflicts)} ${summary.conflicts === 1 ? 'collides' : 'collide'} with existing keys.`;
  return (
    <Summary
      text={`Checked ${rowCount(summary.rows)}: ${count(summary.failed)} with errors. ${conflicts}`}
      summary={summary}
    />
  );
}

function Summary(props: {
  text: string;
  summary: ImportSummaryDto | null;
  alert?: boolean;
}): React.JSX.Element {
  const { summary } = props;
  return (
    <div className={styles.status} role={props.alert ? 'alert' : 'status'}>
      <p className={props.alert ? styles.error : undefined}>{props.text}</p>
      {summary && (
        <p className={styles.note}>
          {count(summary.inserted)} inserted · {count(summary.skipped)} skipped
          · {count(summary.failed)} failed
        </p>
      )}
      {summary && summary.errors.length > 0 && (
        <ErrorReport errors={summary.errors} failed={summary.failed ?? 0} />
      )}
    </div>
  );
}

function ErrorReport(props: {
  errors: readonly RowErrorDto[];
  failed: number;
}): React.JSX.Element {
  const more = props.failed - props.errors.length;
  return (
    <div className={styles.errors}>
      <ul aria-label="Row errors">
        {props.errors.map((error, index) => (
          <li key={String(index)}>
            <span className={styles.line}>Line {error.line}</span>
            {error.column && (
              <span className={styles.category}>{error.column}</span>
            )}
            <span>{error.message}</span>
          </li>
        ))}
      </ul>
      {more > 0 && (
        <p className={styles.note}>…and {more.toLocaleString()} more.</p>
      )}
    </div>
  );
}
