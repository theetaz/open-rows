// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useState } from 'react';
import { Button, TextField } from '@openrows/ui';
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
import { CommandList, ToolList, useDumpContext } from './ToolList';
import styles from './Dump.module.css';

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

function Confirm(props: {
  check: RestoreCheckDto;
  typed: string;
  onTyped: (typed: string) => void;
}): React.JSX.Element {
  const { check } = props;
  if (check.blocked)
    return (
      <p className={styles.error} role="alert">
        {check.blocked}
      </p>
    );
  return (
    <>
      <CommandList commands={check.commands} />
      {check.warnings.map((warning) => (
        <p key={warning} className={styles.warning} role="status">
          {warning}
        </p>
      ))}
      {check.typed && (
        <TextField
          label={`Type ${check.database} to confirm the restore`}
          spellCheck={false}
          autoComplete="off"
          value={props.typed}
          onChange={(event) => {
            props.onTyped(event.target.value);
          }}
        />
      )}
    </>
  );
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
        Choose File…
      </Button>
      {props.folders && (
        <Button
          onClick={() => {
            choose(true);
          }}
        >
          Choose Folder…
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
    title: `Restore ${check.database}`,
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
  entries: Loaded<ArchiveEntryDto[]>;
}): React.JSX.Element {
  const { context, file, request, onRequest, entries } = props;
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
      {entries.status === 'failed' && (
        <p className={styles.error} role="alert">
          {entries.message}
        </p>
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
  const [request, setRequest] = useState<RestoreRequest>(() => ({
    scope: scopeOf(target),
    path: file.path,
    target: { kind: 'existing' },
    options: defaultRestoreOptions,
    confirmToken: null,
    typedName: null,
  }));
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
    <Sheet title={`Restore ${file.name}`} onClose={close}>
      <p className={styles.summary}>
        {FILE_KINDS[file.kind]}
        {file.bytes ? ` · ${sizeText(file.bytes)}` : ''} into{' '}
        {target.connection} : {target.database}
      </p>
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
          entries={entries}
        />
      )}
      {check.status === 'failed' && ready && (
        <p className={styles.error} role="alert">
          {check.message}
        </p>
      )}
      {checked && <Confirm check={checked} typed={typed} onTyped={setTyped} />}
      <SheetActions
        leading={<ChooseOther target={target} folders={postgres} />}
      >
        <Button onClick={close}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!runnable}
          onClick={() => {
            if (checked) restore(request, checked, typed);
          }}
        >
          Restore
        </Button>
      </SheetActions>
    </Sheet>
  );
}
