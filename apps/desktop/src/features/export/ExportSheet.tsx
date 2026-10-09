// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { ExportSourceDto } from '@openrows/ipc';
import { ExportForm } from './ExportForm';
import { isBusy } from './job';
import type { ExportJob } from './job';
import { defaultOptions, formatDto } from './options';
import { useExportJob } from './useExportJob';
import styles from './Export.module.css';

export type ExportSheetProps = {
  title: string;
  /** Suggested file name without extension. */
  fileName: string;
  /** Default target table of SQL INSERT statements. */
  defaultTable: string;
  /** The rows to export. */
  source: ExportSourceDto;
  /**
   * Offered when the row limit cut the received rows: running the
   * statement again exports every row.
   */
  full?: { source: ExportSourceDto; received: number } | undefined;
  onClose: () => void;
};

const count = (rows: number): string =>
  `${rows.toLocaleString()} ${rows === 1 ? 'row' : 'rows'}`;

/**
 * Export options, then the native save panel; progress with Cancel while
 * the core streams, and a notice with the row count when it is done.
 */
export function ExportSheet(props: ExportSheetProps): React.JSX.Element {
  const [options, setOptions] = useState(() =>
    defaultOptions(props.defaultTable),
  );
  const [everyRow, setEveryRow] = useState(false);
  const { job, start, cancel } = useExportJob();
  const format = formatDto(options);
  const busy = isBusy(job);
  const choosing = job.status === 'idle' || job.status === 'failed';
  const run = (): void => {
    if (!format) return;
    const source = everyRow && props.full ? props.full.source : props.source;
    start({ source, format, fileName: props.fileName });
  };
  return (
    <Sheet title={props.title} onClose={busy ? cancel : props.onClose}>
      {!choosing ? (
        <JobStatus job={job} />
      ) : (
        <>
          {props.full && (
            <Scope
              received={props.full.received}
              everyRow={everyRow}
              onEveryRow={setEveryRow}
            />
          )}
          <ExportForm options={options} onOptions={setOptions} />
          {job.status === 'failed' && (
            <p className={styles.error} role="alert">
              {job.message}
            </p>
          )}
        </>
      )}
      <Actions
        job={job}
        ready={!!format}
        onRun={run}
        onCancel={cancel}
        onClose={props.onClose}
      />
    </Sheet>
  );
}

function Actions(props: {
  job: ExportJob;
  ready: boolean;
  onRun: () => void;
  onCancel: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const { job } = props;
  if (job.status === 'finished' || job.status === 'cancelled')
    return (
      <SheetActions>
        <Button variant="primary" onClick={props.onClose}>
          Done
        </Button>
      </SheetActions>
    );
  if (isBusy(job))
    return (
      <SheetActions>
        <Button
          onClick={props.onCancel}
          disabled={job.status !== 'running' || !job.exportId}
        >
          Cancel export
        </Button>
      </SheetActions>
    );
  return (
    <SheetActions>
      <Button onClick={props.onClose}>Cancel</Button>
      <Button variant="primary" disabled={!props.ready} onClick={props.onRun}>
        Export…
      </Button>
    </SheetActions>
  );
}

function Scope(props: {
  received: number;
  everyRow: boolean;
  onEveryRow: (everyRow: boolean) => void;
}): React.JSX.Element {
  return (
    <fieldset className={styles.scope}>
      <legend>Rows</legend>
      <label>
        <input
          type="radio"
          name="export-scope"
          checked={!props.everyRow}
          onChange={() => {
            props.onEveryRow(false);
          }}
        />
        The {count(props.received)} received (row limit)
      </label>
      <label>
        <input
          type="radio"
          name="export-scope"
          checked={props.everyRow}
          onChange={() => {
            props.onEveryRow(true);
          }}
        />
        Every row: run the statement again
      </label>
    </fieldset>
  );
}

function JobStatus({ job }: { job: ExportJob }): React.JSX.Element {
  switch (job.status) {
    case 'finished':
      return (
        <div className={styles.status} role="status">
          <p>
            Exported {count(job.rows)} to {job.fileName}.
          </p>
          <p className={styles.path}>{job.path}</p>
        </div>
      );
    case 'cancelled':
      return (
        <p className={styles.status} role="status">
          Export cancelled. No file was written.
        </p>
      );
    case 'running':
      return (
        <div className={styles.status} role="status">
          <p>Exporting{job.fileName ? ` to ${job.fileName}` : ''}…</p>
          <progress aria-label="Export progress" />
          <p className={styles.note}>{count(job.rows)} written</p>
        </div>
      );
    default:
      return (
        <p className={styles.status} role="status">
          Choose where to save the file…
        </p>
      );
  }
}
