// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import type { CompletionCatalog, FormatKind } from '@openrows/editor';
import { QueryEditor } from './QueryEditor';
import { EditorBar } from './EditorBar';
import { FileChangedBar } from './FileChangedBar';
import { ResultArea } from './ResultArea';
import { ConfirmRun } from './ConfirmRun';
import { ParamsSheet } from './ParamsSheet';
import { NameSheet } from './NameSheet';
import { createCursorStore } from './cursor';
import type { CursorStore } from './cursor';
import { useEditorSettings } from './editorSettings';
import type { EditorSettings } from './editorSettings';
import { isActive } from './run';
import type { RunState } from './run';
import type { EditorHandle, FileAction } from './request';
import { useRunActions } from './useRunActions';
import type { RunActions } from './useRunActions';
import type { RunMenu } from './runMenu';
import { statusText } from './status';
import { addSavedQuery } from './savedQueries';
import { tabKey, useTabAction } from './tabKeys';
import { useTabFiles } from './useTabFiles';
import type { TabFileActions } from './useTabFiles';
import type { QueryTab as Tab } from './tabs';
import type { QueryRuns } from './useQueryRuns';
import type { QueryTabs } from './useQueryTabs';
import type { SelectedCell } from './cell';
import styles from './Query.module.css';

type QueryTabProps = {
  tab: Tab;
  connection: ProfileDto;
  /** The connection's open session, or `null` while disconnected. */
  sessionId: string | null;
  /** Why the tab cannot run even though the connection is open. */
  notice: string | null;
  /** Schema data for completion; `null` while disconnected. */
  catalog: CompletionCatalog | null;
  runs: QueryRuns;
  /** Tab-set actions: text, files and parameter values. */
  tabs: TabFileActions & Pick<QueryTabs, 'setParams'>;
  onCell: (cell: SelectedCell) => void;
  onConnect: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Beautify/uglify through the editor; a formatter error becomes a notice. */
function useFormat(
  editor: { current: EditorHandle | null },
  settings: EditorSettings,
  dialect: ProfileDto['engine'],
): [string | null, (kind: FormatKind) => void] {
  const [failure, setFailure] = useState<string | null>(null);
  const format = (kind: FormatKind): void => {
    const { indentWidth, indentWith, keywordCase, indentStyle } = settings;
    const options = { dialect, indentWidth, indentWith, keywordCase };
    editor.current?.format(kind, { ...options, indentStyle }).then(
      () => {
        setFailure(null);
      },
      (error: unknown) => {
        setFailure(`Could not format: ${message(error)}`);
      },
    );
  };
  return [failure, format];
}

/** Run's dropdown wired to the tab's run, settings and file actions. */
function runMenuOf(input: {
  ready: boolean;
  settings: EditorSettings;
  update: (change: Partial<EditorSettings>) => void;
  actions: RunActions;
  file: (action: FileAction) => void;
  onName: () => void;
}): RunMenu {
  return {
    ready: input.ready,
    runDefault: input.settings.runDefault,
    onRun: input.actions.run,
    onDefault: (scope) => {
      input.update({ runDefault: scope });
    },
    onSaveAs: () => {
      input.file('saveAs');
    },
    onOpenFile: () => {
      input.file('open');
    },
    onAddToQueries: input.onName,
  };
}

/** Editor, bottom bar, results and status for one query tab. */
export function QueryTab(props: QueryTabProps): React.JSX.Element {
  const { tab, runs } = props;
  const editor = useRef<EditorHandle | null>(null);
  const [cursor] = useState(createCursorStore);
  const [settings, updateSettings] = useEditorSettings();
  const [naming, setNaming] = useState(false);
  const dialect = props.connection.engine;
  const [formatFailure, format] = useFormat(editor, settings, dialect);
  const files = useTabFiles(tab, editor, props.tabs);
  const run = runs.runOf(tab.id);
  const actions = useRunActions(editor, settings, props);
  const [act, onExportable] = useTabAction({
    actions,
    format,
    files,
    runs,
    tabId: tab.id,
  });
  return (
    <section
      className={styles.query}
      aria-label={tab.title}
      onKeyDown={(event) => {
        tabKey(event, act);
      }}
    >
      <FileChangedBar tab={tab} editor={editor} tabs={props.tabs} />
      <QueryEditor
        doc={tab.sql}
        label="SQL editor"
        dialect={dialect}
        settings={settings}
        catalog={props.catalog}
        cursor={cursor}
        onChange={(sql) => {
          props.tabs.edit(tab.id, sql);
        }}
        onKey={(event) => tabKey(event, act)}
        onReady={(handle) => {
          editor.current = handle;
        }}
      />
      <TabBar
        {...props}
        settings={settings}
        update={updateSettings}
        cursor={cursor}
        run={run}
        actions={actions}
        file={files.act}
        format={format}
        notice={props.notice ?? formatFailure ?? files.failure}
        onName={() => {
          setNaming(true);
        }}
      />
      <TabResults
        {...props}
        run={run}
        settings={settings}
        onExportable={onExportable}
      />
      <TabSheets
        {...props}
        naming={naming}
        onNamed={() => {
          setNaming(false);
        }}
        onProvide={actions.provide}
        text={() => editor.current?.text() ?? tab.sql}
      />
    </section>
  );
}

/** The bottom bar: editor settings, Run and its menu, Cancel and formatting. */
function TabBar(
  props: Pick<
    QueryTabProps,
    'tab' | 'connection' | 'runs' | 'sessionId' | 'onConnect'
  > & {
    settings: EditorSettings;
    update: (change: Partial<EditorSettings>) => void;
    cursor: CursorStore;
    run: RunState;
    actions: RunActions;
    file: (action: FileAction) => void;
    format: (kind: FormatKind) => void;
    notice: string | null;
    onName: () => void;
  },
): React.JSX.Element {
  const { settings, actions, run } = props;
  const active = isActive(run);
  return (
    <EditorBar
      settings={settings}
      onSettings={props.update}
      dialect={props.connection.engine}
      cursor={props.cursor}
      running={active}
      cancellable={run.queryId !== null}
      notice={props.notice}
      offline={props.sessionId === null}
      onConnect={props.onConnect}
      run={runMenuOf({
        ready: props.sessionId !== null && !active,
        settings,
        update: props.update,
        actions,
        file: props.file,
        onName: props.onName,
      })}
      onRunDefault={() => {
        actions.runKey('current');
      }}
      onCancel={() => {
        props.runs.cancel(props.tab.id);
      }}
      onFormat={props.format}
    />
  );
}

/** Results of the latest run and its one-line status. */
function TabResults(
  props: Pick<QueryTabProps, 'sessionId' | 'onCell' | 'connection'> & {
    run: RunState;
    settings: EditorSettings;
    onExportable: (exportShown: (() => void) | null) => void;
  },
): React.JSX.Element {
  return (
    <>
      <ResultArea
        run={props.run}
        view={props.settings.resultView}
        runDefault={props.settings.runDefault}
        sessionId={props.sessionId}
        engine={props.connection.engine}
        onCell={props.onCell}
        onExportable={props.onExportable}
      />
      <footer className={styles.status} role="status" aria-label="Query status">
        {statusText(props.run)}
      </footer>
    </>
  );
}

/** Confirmation, parameter values and the Add to Queries name, when asked for. */
function TabSheets(
  props: QueryTabProps & {
    naming: boolean;
    onNamed: () => void;
    onProvide: (values: Readonly<Record<string, string>>) => void;
    text: () => string;
  },
): React.JSX.Element | null {
  const { tab, runs } = props;
  const run = runs.runOf(tab.id);
  const dismiss = (): void => {
    runs.dismiss(tab.id);
  };
  if (props.naming)
    return (
      <NameSheet
        title="Add to Queries"
        label="Query name"
        initial={tab.title}
        action="Add"
        onSubmit={(name) =>
          addSavedQuery(props.connection.id, name, props.text())
        }
        onClose={props.onNamed}
      />
    );
  if (run.status === 'parameters' && run.parameters)
    return (
      <ParamsSheet
        names={run.parameters.names}
        last={tab.params ?? {}}
        onRun={props.onProvide}
        onCancel={dismiss}
      />
    );
  if (run.status === 'confirm')
    return (
      <ConfirmRun
        connection={props.connection.name}
        statements={run.statements}
        onConfirm={() => {
          runs.confirm(tab.id);
        }}
        onCancel={dismiss}
      />
    );
  return null;
}
