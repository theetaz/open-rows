// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { checkRunFile } from '@openrows/ipc';
import type { PickedScriptDto, RunFileCheckDto } from '@openrows/ipc';
import { showRunFile } from './runFileStore';
import type { RunTarget } from './runFileStore';
import { sizeText, startFileRun } from './runFileJob';
import styles from './RunFile.module.css';
import { rich, t } from './lazyMessages';

type Check =
  | { status: 'checking' }
  | { status: 'ready'; check: RunFileCheckDto }
  | { status: 'failed'; message: string };

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

function useCheck(file: PickedScriptDto, target: RunTarget): Check {
  const [check, setCheck] = useState<Check>({ status: 'checking' });
  useEffect(() => {
    let live = true;
    checkRunFile({ sessionId: target.sessionId, path: file.path }).then(
      (result) => {
        if (live) setCheck({ status: 'ready', check: result });
      },
      (error: unknown) => {
        if (live) setCheck({ status: 'failed', message: messageOf(error) });
      },
    );
    return () => {
      live = false;
    };
  }, [file.path, target.sessionId]);
  return check;
}

/**
 * A picked `.sql` file before it runs: the safety check of every statement,
 * then the error policy and the transaction option.
 */
export function RunFileSetup(props: {
  file: PickedScriptDto;
  target: RunTarget;
}): React.JSX.Element {
  const { file, target } = props;
  const check = useCheck(file, target);
  const [stopOnError, setStopOnError] = useState(true);
  const [transaction, setTransaction] = useState(false);
  const ready = check.status === 'ready' ? check.check : null;
  const runnable =
    ready !== null && ready.gate !== 'block' && (ready.statements ?? 0) > 0;
  const close = (): void => {
    showRunFile(null);
  };
  const run = (): void => {
    if (!ready || !runnable) return;
    const id = startFileRun({
      file,
      target,
      total: ready.statements ?? 0,
      options: {
        stopOnError: stopOnError || transaction,
        transaction,
        confirmToken: ready.confirmToken,
      },
    });
    showRunFile({ kind: 'run', id });
  };
  return (
    <Sheet title={t('runName', { name: file.name })} onClose={close}>
      <p className={styles.summary}>
        {ready
          ? t('bytesConnectionDatabase', {
              bytes: sizeText(file.bytes ?? 0),
              connection: target.connection,
              database: target.database,
              statements: (ready.statements ?? 0).toLocaleString(),
            })
          : t('bytesConnectionDatabase2', {
              bytes: sizeText(file.bytes ?? 0),
              connection: target.connection,
              database: target.database,
            })}
      </p>
      <CheckResult check={check} />
      {ready && ready.gate !== 'block' && (
        <Options
          check={ready}
          stopOnError={stopOnError || transaction}
          transaction={transaction}
          onStopOnError={setStopOnError}
          onTransaction={setTransaction}
        />
      )}
      <SheetActions>
        <Button onClick={close}>{t('cancel')}</Button>
        <Button variant="primary" disabled={!runnable} onClick={run}>
          {ready?.gate === 'confirm' ? t('runAnyway') : t('run')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function CheckResult({ check }: { check: Check }): React.JSX.Element | null {
  if (check.status === 'checking')
    return (
      <p className={styles.note} role="status">
        {t('checkingEveryStatement')}
      </p>
    );
  if (check.status === 'failed')
    return (
      <p className={styles.error} role="alert">
        {check.message}
      </p>
    );
  const { check: result } = check;
  if ((result.statements ?? 0) === 0)
    return <p className={styles.note}>{t('fileHoldsNo')}</p>;
  if (result.gate === 'run') return null;
  const blocked = result.gate === 'block';
  return (
    <div className={styles.safety} role={blocked ? 'alert' : 'status'}>
      <p>
        {blocked
          ? t('connectionReadOnly', {
              blocked: (result.blocked ?? 0).toLocaleString(),
            })
          : t('confirmStatementsMay', {
              confirm: (result.confirm ?? 0).toLocaleString(),
            })}
      </p>
      <ul className={styles.samples}>
        {result.samples.map((sample) => (
          <li key={`${String(sample.line)}:${sample.preview}`}>
            <span className={styles.line}>
              {rich('lineLine', { line: sample.line })}
            </span>
            <code>{sample.preview}</code>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Options(props: {
  check: RunFileCheckDto;
  stopOnError: boolean;
  transaction: boolean;
  onStopOnError: (stop: boolean) => void;
  onTransaction: (transaction: boolean) => void;
}): React.JSX.Element {
  return (
    <fieldset className={styles.options}>
      <legend>{t('error')}</legend>
      <label>
        <input
          type="radio"
          name="run-file-errors"
          checked={props.stopOnError}
          onChange={() => {
            props.onStopOnError(true);
          }}
        />
        {t('stopFirstError')}
      </label>
      <label>
        <input
          type="radio"
          name="run-file-errors"
          checked={!props.stopOnError}
          disabled={props.transaction}
          onChange={() => {
            props.onStopOnError(false);
          }}
        />
        {t('continueListEvery')}
      </label>
      <label>
        <input
          type="checkbox"
          checked={props.transaction}
          disabled={!props.check.transactions}
          onChange={(event) => {
            props.onTransaction(event.target.checked);
          }}
        />
        {t('runEverythingOne')}
      </label>
      {!props.check.transactions && (
        <p className={styles.note}>{t('databaseCommitsSchema')}</p>
      )}
    </fieldset>
  );
}
