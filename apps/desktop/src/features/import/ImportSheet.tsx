// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { confirmDestructive } from '@openrows/ipc';
import type { ImportRequest } from '@openrows/ipc';
import { ImportStatus } from './ImportStatus';
import { ImportSetup } from './ImportSetup';
import { isBusy } from './job';
import type { ImportJob } from './job';
import { draftProblem, importRequest } from './model';
import type { ImportContext, ImportDraft } from './model';
import { useImportDraft } from './useImportDraft';
import { useImportJob } from './useImportJob';
import styles from './Import.module.css';
import { t } from './messages';

export type ImportSheetProps = {
  context: ImportContext;
  onClose: () => void;
};

/** What the confirmation says before rows are written. */
export function confirmation(
  draft: ImportDraft,
  context: ImportContext,
): { title: string; message: string; action: string } {
  const table = `${context.schema}.${draft.table.trim()}`;
  const steps = [
    t('rowsFilenameWritten', {
      fileName: draft.file?.fileName ?? t('file2'),
      table,
    }),
    draft.target === 'new' ? t('tableCreatedFirst') : '',
    draft.target === 'existing' && draft.truncate ? t('everyExistingRow') : '',
  ];
  return {
    title: t('importIntoTable', { table: draft.table.trim() }),
    message: steps.filter(Boolean).join(' '),
    action: t('import'),
  };
}

/** True once an import (not a dry run) started and has something to show. */
const showsImport = (job: ImportJob): boolean =>
  job.status !== 'idle' &&
  job.mode === 'import' &&
  !(job.status === 'failed' && job.summary === null);

/**
 * Import a CSV, TSV, JSON or NDJSON file into a table: pick the file, map
 * its columns, preview types, check conflicts, then confirm and watch the
 * background job until its summary.
 */
export function ImportSheet({
  context,
  onClose,
}: ImportSheetProps): React.JSX.Element {
  const state = useImportDraft(context);
  const { job, start, cancel, reset } = useImportJob();
  const [confirming, setConfirming] = useState(false);
  const run = (mode: ImportRequest['mode']): void => {
    setConfirming(false);
    const request = importRequest(state.draft, context, mode);
    if (request) start(request);
  };
  const ask = (): void => {
    confirmDestructive(confirmation(state.draft, context)).then(
      (answer) => {
        if (answer === null) setConfirming(true);
        else if (answer) run('import');
      },
      () => {
        setConfirming(true);
      },
    );
  };
  const importing = showsImport(job);
  return (
    <Sheet
      title={t('importIntoDatabase', {
        database: context.database,
        schema: context.schema,
      })}
      size="editor"
      onClose={isBusy(job) ? cancel : onClose}
    >
      <div className={styles.body}>
        {importing && <ImportStatus job={job} table={state.draft.table} />}
        {!importing && confirming && (
          <p className={styles.confirm} role="alert">
            {confirmation(state.draft, context).message}
          </p>
        )}
        {!importing && !confirming && (
          <ImportSetup state={state} job={job} context={context} />
        )}
      </div>
      <Actions
        job={job}
        importing={importing}
        confirming={confirming}
        problem={
          context.readOnly ? t('connectionReadOnly') : draftProblem(state.draft)
        }
        checkable={!draftProblem(state.draft)}
        on={{
          check: () => {
            run('check');
          },
          import: ask,
          confirm: () => {
            run('import');
          },
          back: () => {
            setConfirming(false);
            reset();
          },
          cancel,
          close: onClose,
        }}
      />
    </Sheet>
  );
}

type Handlers = Record<
  'check' | 'import' | 'confirm' | 'back' | 'cancel' | 'close',
  () => void
>;

function Actions(props: {
  job: ImportJob;
  importing: boolean;
  confirming: boolean;
  problem: string | null;
  checkable: boolean;
  on: Handlers;
}): React.JSX.Element {
  const { job, on } = props;
  if (job.status === 'starting' || job.status === 'running')
    return (
      <SheetActions>
        <Button
          onClick={on.cancel}
          disabled={job.status !== 'running' || !job.importId}
        >
          {job.mode === 'check' ? t('stopCheck') : t('cancelImport')}
        </Button>
      </SheetActions>
    );
  if (props.importing)
    return (
      <SheetActions>
        {job.status === 'failed' && (
          <Button onClick={on.back}>{t('back')}</Button>
        )}
        <Button variant="primary" onClick={on.close}>
          {t('done')}
        </Button>
      </SheetActions>
    );
  if (props.confirming)
    return (
      <SheetActions>
        <Button onClick={on.back}>{t('back')}</Button>
        <Button variant="destructive" onClick={on.confirm}>
          {t('import')}
        </Button>
      </SheetActions>
    );
  return (
    <SheetActions
      leading={
        props.problem && <span className={styles.note}>{props.problem}</span>
      }
    >
      <Button onClick={on.close}>{t('cancel')}</Button>
      <Button onClick={on.check} disabled={!props.checkable}>
        {t('checkConflicts')}
      </Button>
      <Button
        variant="primary"
        onClick={on.import}
        disabled={Boolean(props.problem)}
      >
        {t('import2')}
      </Button>
    </SheetActions>
  );
}
