// SPDX-License-Identifier: Apache-2.0
import type { ImportSummaryDto, RowErrorDto } from '@openrows/ipc';
import { rowCount } from './job';
import type { ImportJob, ImportProgress } from './job';
import styles from './Import.module.css';
import { rich, t, tn } from './messages';

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
          {job.mode === 'check' ? t('checkingFile') : t('startingImport')}
        </p>
      );
    case 'running':
      return <Running mode={job.mode} progress={job.progress} />;
    case 'finished':
      return job.mode === 'check' ? (
        <CheckSummary summary={job.summary} />
      ) : (
        <Summary
          text={t('importedInsertedInto', {
            inserted: rowCount(job.summary.inserted),
            table: props.table,
          })}
          summary={job.summary}
        />
      );
    case 'cancelled':
      return (
        <Summary
          text={
            job.mode === 'check'
              ? t('checkCancelled')
              : t('importCancelledUncommitted')
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
      <p>{props.mode === 'check' ? t('checking') : t('importing')}</p>
      <progress
        aria-label={
          props.mode === 'check' ? t('checkProgress') : t('importProgress')
        }
        max={1}
        value={progress.fraction}
      />
      <p className={styles.note}>
        {t('mathRowsRead', {
          percent: Math.round(progress.fraction * 100),
          rows: rowCount(progress.rows),
          inserted: count(progress.inserted),
          failed: count(progress.failed),
          rowsPerSecond: count(progress.rowsPerSecond),
        })}
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
      ? t('conflictsCannotChecked')
      : tn('countCollideExisting', summary.conflicts, {
          count: rowCount(summary.conflicts),
        });
  return (
    <Summary
      text={t('checkedRowsFailed', {
        rows: rowCount(summary.rows),
        failed: count(summary.failed),
        conflicts,
      })}
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
          {t('insertedInsertedSkipped', {
            inserted: count(summary.inserted),
            skipped: count(summary.skipped),
            failed: count(summary.failed),
          })}
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
      <ul aria-label={t('rowErrors')}>
        {props.errors.map((error, index) => (
          <li key={String(index)}>
            <span className={styles.line}>
              {rich('lineLine', { line: error.line })}
            </span>
            {error.column && (
              <span className={styles.category}>{error.column}</span>
            )}
            <span>{error.message}</span>
          </li>
        ))}
      </ul>
      {more > 0 && (
        <p className={styles.note}>
          {t('moreMore', { more: more.toLocaleString() })}
        </p>
      )}
    </div>
  );
}
