// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import type { CompletionCatalog, FormatKind } from '@openrows/editor';
import { QueryEditor } from './QueryEditor';
import { EditorBar } from './EditorBar';
import { ResultPane } from './ResultPane';
import { ConfirmRun } from './ConfirmRun';
import { createCursorStore } from './cursor';
import { useEditorSettings } from './editorSettings';
import type { EditorSettings } from './editorSettings';
import { isActive } from './run';
import { formatKindOf, runRequest, runScopeOf } from './request';
import type { EditorHandle, RunScope } from './request';
import { statusText } from './status';
import type { QueryTab as Tab } from './tabs';
import type { QueryRuns } from './useQueryRuns';
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
  onSql: (tabId: string, sql: string) => void;
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
    const { indentWidth, indentWith, keywordCase } = settings;
    const options = { dialect, indentWidth, indentWith, keywordCase };
    editor.current?.format(kind, options).then(
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

/** Run (⌘↵, ⇧⌘↵) and format (⌘I, ⇧⌘I) shortcuts anywhere in the tab. */
function shortcut(
  event: KeyboardEvent<HTMLElement>,
  start: (scope: RunScope) => void,
  format: (kind: FormatKind) => void,
): void {
  const scope = runScopeOf(event);
  const kind = formatKindOf(event);
  if (scope) start(scope);
  else if (kind) format(kind);
  else return;
  event.preventDefault();
}

/** Editor, bottom bar, results and status for one query tab. */
export function QueryTab(props: QueryTabProps): React.JSX.Element {
  const { tab, runs, sessionId } = props;
  const editor = useRef<EditorHandle | null>(null);
  const [cursor] = useState(createCursorStore);
  const [settings, updateSettings] = useEditorSettings();
  const dialect = props.connection.engine;
  const [formatFailure, format] = useFormat(editor, settings, dialect);
  const run = runs.runOf(tab.id);
  const start = (scope: RunScope): void => {
    const handle = editor.current;
    if (!handle || !sessionId || isActive(run)) return;
    const request = runRequest(handle, scope, sessionId, settings.rowLimit);
    if (!request) return;
    props.onSql(tab.id, handle.text());
    void runs.execute(tab.id, request);
  };
  const active = isActive(run);
  return (
    <section
      className={styles.query}
      aria-label={tab.title}
      onKeyDown={(event) => {
        shortcut(event, start, format);
      }}
    >
      <QueryEditor
        doc={tab.sql}
        label="SQL editor"
        dialect={dialect}
        settings={settings}
        catalog={props.catalog}
        cursor={cursor}
        onChange={(sql) => {
          props.onSql(tab.id, sql);
        }}
        onRun={start}
        onFormat={format}
        onReady={(handle) => {
          editor.current = handle;
        }}
      />
      <EditorBar
        settings={settings}
        onSettings={updateSettings}
        cursor={cursor}
        running={active}
        ready={sessionId !== null && !active}
        cancellable={run.queryId !== null}
        notice={props.notice ?? formatFailure}
        offline={sessionId === null}
        onConnect={props.onConnect}
        onRun={start}
        onCancel={() => {
          runs.cancel(tab.id);
        }}
        onFormat={format}
      />
      <ResultPane run={run} onCell={props.onCell} />
      <footer className={styles.status} role="status" aria-label="Query status">
        {statusText(run)}
      </footer>
      {run.status === 'confirm' && (
        <ConfirmRun
          connection={props.connection.name}
          statements={run.statements}
          onConfirm={() => {
            runs.confirm(tab.id);
          }}
          onCancel={() => {
            runs.dismiss(tab.id);
          }}
        />
      )}
    </section>
  );
}
