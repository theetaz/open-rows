// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import type { CompletionCatalog, FormatKind } from '@openrows/editor';
import { useHistoryInsert } from './historyInsert';
import { QueryEditor } from './QueryEditor';
import { EditorBar } from './EditorBar';
import { FileChangedBar } from './FileChangedBar';
import { ResultArea } from './ResultArea';
import { ConfirmRun } from './ConfirmRun';
import { TabParams } from './TabParams';
import { TabSnippets } from './TabSnippets';
import { NameSheet } from './NameSheet';
import { createCursorStore } from './cursor';
import type { CursorStore } from './cursor';
import { useEditorSettings } from './editorSettings';
import { useFormat } from './useFormat';
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
import type { TabCommand } from '../commands';
import type { SelectedCell } from './cell';
import { ExplainSheet, explainResultTab, useExplain } from '../explain';
import type { ExplainControls } from '../explain';
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
    onRunUnlimited: input.actions.runUnlimited,
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

/** A sheet's open state with stable open and close callbacks. */
function useFlag(): [boolean, () => void, () => void] {
  const [on, setOn] = useState(false);
  const open = useCallback(() => {
    setOn(true);
  }, []);
  const close = useCallback(() => {
    setOn(false);
  }, []);
  return [on, open, close];
}

/** Editor, bottom bar, results and status for one query tab. */
export function QueryTab(props: QueryTabProps): React.JSX.Element {
  const { tab, runs } = props;
  const editor = useRef<EditorHandle | null>(null);
  const [cursor] = useState(createCursorStore);
  const [settings, updateSettings] = useEditorSettings();
  const [naming, startNaming, stopNaming] = useFlag();
  const [picking, startPicking, stopPicking] = useFlag();
  const dialect = props.connection.engine;
  const [formatFailure, format] = useFormat(editor, settings, dialect);
  const files = useTabFiles(tab, editor, props.tabs);
  const run = runs.runOf(tab.id);
  const actions = useRunActions(editor, settings, props);
  const explain = useExplain(tab.id, props.sessionId, editor);
  const [act, onExportable, claim] = useTabAction({
    actions,
    format,
    files,
    runs,
    tabId: tab.id,
    onSnippet: startPicking,
    onExplain: explain.run,
  });
  return (
    <section
      className={styles.query}
      aria-label={tab.title}
      onFocus={useHistoryInsert(tab.id, editor, claim)}
      onKeyDown={(event) => {
        tabKey(event, act);
      }}
    >
      <TabEditor
        {...props}
        settings={settings}
        cursor={cursor}
        editorRef={editor}
        act={act}
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
        onName={startNaming}
        onExplain={explain.run}
      />
      <TabParams
        {...props}
        dialect={dialect}
        settings={settings}
        run={run}
        onRun={actions.provide}
      />
      <TabResults
        {...props}
        run={run}
        settings={settings}
        onExportable={onExportable}
        explain={explain}
      />
      <TabSheets
        {...props}
        naming={naming}
        onNamed={stopNaming}
        text={() => editor.current?.text() ?? tab.sql}
        picking={picking}
        editorRef={editor}
        onPicked={stopPicking}
      />
    </section>
  );
}

/**
 * The tab's SQL editor, its text reaching the tab descriptor as typed, under
 * the notice of a bound file changed on disk.
 */
function TabEditor(
  props: Pick<QueryTabProps, 'tab' | 'connection' | 'catalog' | 'tabs'> & {
    settings: EditorSettings;
    cursor: CursorStore;
    editorRef: { current: EditorHandle | null };
    act: (id: TabCommand) => void;
  },
): React.JSX.Element {
  const { tab, editorRef, act } = props;
  return (
    <>
      {tab.sql.toUpperCase().includes('__OPENROWS_REDACTED_') && (
        <p role="status">
          History literals were redacted. Fill every placeholder before running
          SQL.
        </p>
      )}
      <FileChangedBar tab={tab} editor={editorRef} tabs={props.tabs} />
      <QueryEditor
        doc={tab.sql}
        label="SQL editor"
        dialect={props.connection.engine}
        settings={props.settings}
        catalog={props.catalog}
        cursor={props.cursor}
        onChange={(sql) => {
          props.tabs.edit(tab.id, sql);
        }}
        onKey={(event) => tabKey(event, act)}
        onReady={(handle) => {
          editorRef.current = handle;
        }}
      />
    </>
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
    onExplain: (analyze: boolean) => void;
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
      onExplain={props.onExplain}
    />
  );
}

/** Results of the latest run and its one-line status. */
function TabResults(
  props: Pick<QueryTabProps, 'sessionId' | 'onCell' | 'connection'> & {
    run: RunState;
    settings: EditorSettings;
    onExportable: (exportShown: (() => void) | null) => void;
    explain: ExplainControls;
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
        extra={explainResultTab(props.explain)}
      />
      <ExplainSheet
        explain={props.explain}
        connection={props.connection.name}
      />
      <footer className={styles.status} role="status" aria-label="Query status">
        {statusText(props.run)}
      </footer>
    </>
  );
}

/** Confirmation, the Add to Queries name and Insert Snippet…, when asked for. */
function TabSheets(
  props: QueryTabProps & {
    naming: boolean;
    onNamed: () => void;
    text: () => string;
    picking: boolean;
    editorRef: { current: EditorHandle | null };
    onPicked: () => void;
  },
): React.JSX.Element | null {
  const { tab, runs } = props;
  if (props.picking)
    return (
      <TabSnippets
        dialect={props.connection.engine}
        editorRef={props.editorRef}
        onClose={props.onPicked}
      />
    );
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
