// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { pickBackupFile, previewBackup, startBackup } from '@openrows/ipc';
import type {
  BackupOptionsDto,
  DumpContextDto,
  DumpPreviewDto,
} from '@openrows/ipc';
import { ExcludeTableData } from './ExcludeTableData';
import { BackupScope } from './BackupScope';
import { initialSelection, scopeOptions, useBackupScope } from './scopeModel';
import { BackupOptions } from './BackupOptions';
import { showDump } from './dumpStore';
import type { DumpTarget } from './dumpStore';
import { startDumpJob } from './dumpJob';
import { backupTool, defaultBackupOptions, messageOf, scopeOf } from './model';
import { CommandList, ToolList, useDumpContext } from './ToolList';
import styles from './Dump.module.css';

type Preview =
  | { status: 'loading' }
  | { status: 'ready'; preview: DumpPreviewDto }
  | { status: 'failed'; message: string };

/** The exact command the options give, refreshed on every change. */
function usePreview(
  target: DumpTarget,
  options: BackupOptionsDto,
  ready: boolean,
): Preview {
  const key = JSON.stringify([target, options]);
  const [state, setState] = useState<{ key: string; preview: Preview } | null>(
    null,
  );
  useEffect(() => {
    if (!ready) return;
    let live = true;
    previewBackup({ scope: scopeOf(target), options, output: null }).then(
      (result) => {
        if (live)
          setState({ key, preview: { status: 'ready', preview: result } });
      },
      (error: unknown) => {
        if (live)
          setState({
            key,
            preview: { status: 'failed', message: messageOf(error) },
          });
      },
    );
    return () => {
      live = false;
    };
  }, [target, options, ready, key]);
  return ready && state?.key === key ? state.preview : { status: 'loading' };
}

/** Choose the file, then start the backup as a job and show its log. */
async function backUp(
  target: DumpTarget,
  options: BackupOptionsDto,
): Promise<void> {
  const output = await pickBackupFile({
    sessionId: target.sessionId,
    format: options.format,
  });
  if (!output) return;
  const id = startDumpJob({
    kind: 'backup',
    title: `Back up ${target.database}`,
    start: (onEvent) =>
      startBackup({ scope: scopeOf(target), options, output }, onEvent),
  });
  showDump({ kind: 'log', id });
}

function PreviewResult({
  preview,
}: {
  preview: Preview;
}): React.JSX.Element | null {
  if (preview.status === 'loading') return null;
  if (preview.status === 'failed')
    return (
      <p className={styles.error} role="alert">
        {preview.message}
      </p>
    );
  return (
    <>
      <CommandList commands={preview.preview.commands} />
      {preview.preview.warnings.map((warning) => (
        <p key={warning} className={styles.warning} role="status">
          {warning}
        </p>
      ))}
    </>
  );
}

function Body(props: {
  context: DumpContextDto;
  options: BackupOptionsDto;
  onOptions: (options: BackupOptionsDto) => void;
  preview: Preview;
}): React.JSX.Element {
  const { context } = props;
  const tool = backupTool(context);
  if (!tool)
    return (
      <p className={styles.note}>
        The database file is copied with SQLite&apos;s own backup (VACUUM INTO),
        consistent even while it is in use.
      </p>
    );
  return (
    <>
      <ToolList tools={context.tools} kinds={[tool]} />
      <BackupOptions
        engine={context.engine}
        options={props.options}
        onChange={props.onOptions}
      />
      <PreviewResult preview={props.preview} />
    </>
  );
}

/**
 * Back Up Database…: the tool and its version, the options, and the exact
 * command before the save panel; the backup then runs as a job.
 */
export function BackupSheet({
  target,
}: {
  target: DumpTarget;
}): React.JSX.Element {
  const {
    state,
    context,
    options,
    preview,
    ready,
    scope,
    selection,
    setSelection,
    settings,
    setOptions,
  } = useBackupForm(target);
  const [failure, setFailure] = useState<string | null>(null);
  const close = (): void => {
    showDump(null);
  };
  return (
    <Sheet title={`Back Up ${target.database}`} onClose={close}>
      <p className={styles.summary}>
        {target.connection} : {target.database}
      </p>
      {state.status === 'loading' && (
        <p className={styles.note} role="status">
          Finding the client tools…
        </p>
      )}
      {state.status === 'failed' && (
        <p className={styles.error} role="alert">
          {state.message}
        </p>
      )}
      {context && context.engine !== 'sqlite' && (
        <ScopeBody
          scope={scope}
          selection={selection}
          onSelection={setSelection}
          settings={settings}
          onOptions={setOptions}
          engine={context.engine}
        />
      )}
      {context && (
        <Body
          context={context}
          options={options}
          onOptions={setOptions}
          preview={preview}
        />
      )}
      {failure && (
        <p className={styles.error} role="alert">
          {failure}
        </p>
      )}
      <SheetActions>
        <Button onClick={close}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!ready}
          onClick={() => {
            backUp(target, options).catch((error: unknown) => {
              setFailure(messageOf(error));
            });
          }}
        >
          Back Up…
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function ScopeBody(props: {
  scope: ReturnType<typeof useBackupScope>;
  selection: ReturnType<typeof initialSelection>;
  onSelection: (selection: ReturnType<typeof initialSelection>) => void;
  settings: BackupOptionsDto;
  onOptions: (options: BackupOptionsDto) => void;
  engine: DumpContextDto['engine'];
}): React.JSX.Element {
  const { scope, selection, settings } = props;
  if (scope.loading) return <p role="status">Loading backup objects…</p>;
  if (scope.failure) return <p role="alert">{scope.failure}</p>;
  return (
    <>
      <BackupScope
        catalog={scope.catalog}
        selection={selection}
        onChange={props.onSelection}
      />
      {props.engine === 'postgres' && (
        <ExcludeTableData
          catalog={scope.catalog}
          selected={settings.excludeTableData ?? []}
          onChange={(excludeTableData) => {
            props.onOptions({ ...settings, excludeTableData });
          }}
        />
      )}
    </>
  );
}

function useBackupForm(target: DumpTarget) {
  const state = useDumpContext(target);
  const [settings, setOptions] = useState(defaultBackupOptions);
  const [selection, setSelection] = useState(() => initialSelection(target));
  const context = state.status === 'ready' ? state.context : null;
  const scope = useBackupScope(target, context?.engine);
  const options = useMemo(
    () => ({ ...settings, ...scopeOptions(selection, scope.catalog) }),
    [settings, selection, scope.catalog],
  );
  const hasScope =
    selection.all ||
    selection.schemas.length > 0 ||
    selection.tables.length > 0;
  const scopeReady =
    context?.engine === 'sqlite' || scopeAvailable(scope, hasScope);
  const preview = usePreview(target, options, context !== null && scopeReady);
  const tool = context && backupTool(context);
  const broken = context?.tools.find((item) => item.tool === tool)?.error;
  const ready =
    !!context && !broken && scopeReady && preview.status === 'ready';
  return {
    state,
    context,
    options,
    preview,
    ready,
    scope,
    selection,
    setSelection,
    settings,
    setOptions,
  };
}

function scopeAvailable(
  scope: ReturnType<typeof useBackupScope>,
  hasScope: boolean,
): boolean {
  return !scope.loading && !scope.failure && hasScope;
}
