// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { checkRestore, restoreContents, startRestore } from '@openrows/ipc';
import type {
  ArchiveEntryDto,
  DumpContextDto,
  PickedDumpDto,
  RestoreCheckDto,
  RestoreRequest,
  RestoreTargetDto,
} from '@openrows/ipc';
import { sizeText } from '../runfile';
import { showDump } from './dumpStore';
import type { DumpTarget } from './dumpStore';
import { startDumpJob } from './dumpJob';
import { pickRestoreFor } from './openDump';
import {
  defaultRestoreOptions,
  FILE_KINDS,
  isArchive,
  messageOf,
  restoreTools,
  scopeOf,
} from './model';
import { ArchiveEntries, RestoreOptions, TargetChoice } from './RestoreOptions';
import { RestoreConfirm } from './RestoreConfirm';
import { ToolList, useDumpContext } from './ToolList';
import styles from './Dump.module.css';
import { t } from './messages';

type Loaded<T> =
  | { status: 'loading' }
  | { status: 'ready'; value: T }
  | { status: 'failed'; message: string };

/** Run `load` whenever `request` changes (and `request` is set). */
function useLoaded<R, T>(
  request: R | null,
  load: (request: R) => Promise<T>,
): Loaded<T> {
  const [state, setState] = useState<{ of: R | null; loaded: Loaded<T> }>({
    of: null,
    loaded: { status: 'loading' },
  });
  useEffect(() => {
    if (request === null) return;
    let live = true;
    const settle = (loaded: Loaded<T>): void => {
      if (live) setState({ of: request, loaded });
    };
    load(request).then(
      (value) => {
        settle({ status: 'ready', value });
      },
      (error: unknown) => {
        settle({ status: 'failed', message: messageOf(error) });
      },
    );
    return () => {
      live = false;
    };
  }, [request, load]);
  return state.of === request ? state.loaded : { status: 'loading' };
}

/** The archive's table of contents (`pg_restore --list`). */
function useContents(
  target: DumpTarget,
  file: PickedDumpDto,
): Loaded<ArchiveEntryDto[]> {
  const request = useMemo(
    () =>
      isArchive(file.kind) ? { scope: scopeOf(target), path: file.path } : null,
    [target, file],
  );
  return useLoaded(request, restoreContents);
}

/** Pick another backup file, or a `pg_dump` directory on PostgreSQL. */
function ChooseOther(props: {
  target: DumpTarget;
  folders: boolean;
}): React.JSX.Element {
  const choose = (folder: boolean): void => {
    // A failed panel has nothing to show; the sheet stays usable.
    pickRestoreFor(props.target, folder).catch(() => undefined);
  };
  return (
    <>
      <Button
        onClick={() => {
          choose(false);
        }}
      >
        {t('chooseFile')}
      </Button>
      {props.folders && (
        <Button
          onClick={() => {
            choose(true);
          }}
        >
          {t('chooseFolder')}
        </Button>
      )}
    </>
  );
}

/** Start a checked restore as a job and show its log. */
function restore(
  request: RestoreRequest,
  check: RestoreCheckDto,
  typed: string,
): void {
  const id = startDumpJob({
    kind: 'restore',
    title: t('restoreDatabase', { database: check.database }),
    start: (onEvent) =>
      startRestore(
        {
          ...request,
          confirmToken: check.confirmToken,
          typedName: check.typed ? typed : null,
        },
        onEvent,
      ),
  });
  showDump({ kind: 'log', id });
}

function Body(props: {
  context: DumpContextDto;
  file: PickedDumpDto;
  request: RestoreRequest;
  onRequest: (request: RestoreRequest) => void;
}): React.JSX.Element {
  const { context, file, request, onRequest } = props;
  return (
    <>
      <ToolList
        tools={context.tools}
        kinds={restoreTools(context, file.kind)}
      />
      <TargetChoice
        engine={context.engine}
        database={context.database}
        target={request.target}
        onChange={(target: RestoreTargetDto) => {
          onRequest({ ...request, target });
        }}
      />
      <RestoreOptions
        kind={file.kind}
        options={request.options}
        onChange={(options) => {
          onRequest({ ...request, options });
        }}
      />
    </>
  );
}

/** The gate's answer: why it failed, or the commands and the confirmation. */
function Checked(props: {
  check: Loaded<RestoreCheckDto>;
  typed: string;
  onTyped: (typed: string) => void;
}): React.JSX.Element | null {
  const { check } = props;
  if (check.status === 'failed')
    return (
      <p className={styles.error} role="alert">
        {check.message}
      </p>
    );
  if (check.status !== 'ready') return null;
  return (
    <RestoreConfirm
      check={check.value}
      typed={props.typed}
      onTyped={props.onTyped}
    />
  );
}

/** The archive's entries to pick, under their subhead. */
function Entries(props: {
  request: RestoreRequest;
  onRequest: (request: RestoreRequest) => void;
  entries: Loaded<ArchiveEntryDto[]>;
}): React.JSX.Element {
  const { request, onRequest, entries } = props;
  return (
    <>
      {entries.status === 'failed' && (
        <p className={styles.error} role="alert">
          {entries.message}
        </p>
      )}
      {entries.status === 'ready' && entries.value.length > 0 && (
        <h3 className={styles.subhead}>{t('archiveContents')}</h3>
      )}
      {entries.status === 'ready' && (
        <ArchiveEntries
          entries={entries.value}
          selected={request.options.selected}
          onChange={(selected) => {
            onRequest({
              ...request,
              options: { ...request.options, selected },
            });
          }}
        />
      )}
    </>
  );
}

function restoreSummary(file: PickedDumpDto, target: DumpTarget): string {
  const into = {
    kind: FILE_KINDS[file.kind],
    connection: target.connection,
    database: target.database,
  };
  return file.bytes
    ? t('fileKindsBytes', { ...into, bytes: sizeText(file.bytes) })
    : t('fileKindsInto', into);
}

/** Into the open database with the default options (or only the chosen schema). */
function initialRequest(
  target: DumpTarget,
  file: PickedDumpDto,
): RestoreRequest {
  return {
    scope: scopeOf(target),
    path: file.path,
    target: { kind: 'existing' },
    options: {
      ...defaultRestoreOptions,
      schemas: target.restoreSchema ? [target.restoreSchema] : [],
    },
    confirmToken: null,
    typedName: null,
  };
}

/**
 * Restore Database…: the picked file, where it goes, the options, the exact
 * commands and the confirmation the gate asks for (the database name on
 * production connections and destructive restores); read-only connections
 * never restore.
 */
export function RestoreSheet(props: {
  target: DumpTarget;
  file: PickedDumpDto;
}): React.JSX.Element {
  const { target, file } = props;
  const context = useDumpContext(target);
  const entries = useContents(target, file);
  const [request, setRequest] = useState(() => initialRequest(target, file));
  const ready = context.status === 'ready' ? context.context : null;
  const check = useLoaded(ready ? request : null, checkRestore);
  const [typed, setTyped] = useState('');
  const checked = check.status === 'ready' ? check.value : null;
  const runnable =
    !!checked &&
    !checked.blocked &&
    (!checked.typed || typed.trim() === checked.database);
  const postgres = ready?.engine === 'postgres';
  const close = (): void => {
    showDump(null);
  };
  return (
    <Sheet
      title={t('restoreName', { name: file.name })}
      size="editor"
      onClose={close}
    >
      <p className={styles.summary}>{restoreSummary(file, target)}</p>
      {target.restoreSchema && (
        <p>{t('restoreOnlySchema', { restoreSchema: target.restoreSchema })}</p>
      )}
      <div className={styles.halves} data-split="">
        <section className={styles.half}>
          {context.status === 'failed' && (
            <p className={styles.error} role="alert">
              {context.message}
            </p>
          )}
          {ready && (
            <Body
              context={ready}
              file={file}
              request={request}
              onRequest={setRequest}
            />
          )}
        </section>
        {ready && (
          <section className={styles.half}>
            <Entries
              request={request}
              onRequest={setRequest}
              entries={entries}
            />
            <Checked check={check} typed={typed} onTyped={setTyped} />
          </section>
        )}
      </div>
      <SheetActions
        leading={<ChooseOther target={target} folders={postgres} />}
      >
        <Button onClick={close}>{t('cancel')}</Button>
        <Button
          variant="primary"
          disabled={!runnable}
          onClick={() => {
            if (checked) restore(request, checked, typed);
          }}
        >
          {t('restore')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
