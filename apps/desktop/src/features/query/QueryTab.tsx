// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import type { KeyboardEvent } from 'react';
import { Button, Kbd } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { QueryEditor } from './QueryEditor';
import { ResultPane } from './ResultPane';
import { ConfirmRun } from './ConfirmRun';
import { isActive } from './run';
import type { RunState } from './run';
import { runRequest, runScopeOf } from './request';
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
  runs: QueryRuns;
  onSql: (tabId: string, sql: string) => void;
  onCell: (cell: SelectedCell) => void;
  onConnect: () => void;
};

/** Editor, run controls, results and status for one query tab. */
export function QueryTab(props: QueryTabProps): React.JSX.Element {
  const { tab, runs, sessionId } = props;
  const editor = useRef<EditorHandle | null>(null);
  const run = runs.runOf(tab.id);
  const start = (scope: RunScope): void => {
    const handle = editor.current;
    if (!handle || !sessionId || isActive(run)) return;
    const request = runRequest(handle, scope, sessionId);
    if (!request) return;
    props.onSql(tab.id, handle.text());
    void runs.execute(tab.id, request);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    const scope = runScopeOf(event);
    if (!scope) return;
    event.preventDefault();
    start(scope);
  };
  return (
    <section
      className={styles.query}
      aria-label={tab.title}
      onKeyDown={onKeyDown}
    >
      <Toolbar {...props} run={run} onRun={start} />
      <QueryEditor
        doc={tab.sql}
        label="SQL editor"
        onChange={(sql) => {
          props.onSql(tab.id, sql);
        }}
        onRun={start}
        onReady={(handle) => {
          editor.current = handle;
        }}
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

function Toolbar(
  props: QueryTabProps & { run: RunState; onRun: (scope: RunScope) => void },
): React.JSX.Element {
  const active = isActive(props.run);
  const ready = props.sessionId !== null && !active;
  return (
    <div className={styles.toolbar} role="toolbar" aria-label="Query actions">
      <Button
        size="sm"
        variant="primary"
        aria-label="Run current statement"
        title="Run current statement or selection (⌘↵)"
        disabled={!ready}
        onClick={() => {
          props.onRun('current');
        }}
      >
        Run{' '}
        <Kbd variant="inline" aria-hidden="true">
          ⌘↵
        </Kbd>
      </Button>
      <Button
        size="sm"
        aria-label="Run all statements"
        title="Run all statements (⇧⌘↵)"
        disabled={!ready}
        onClick={() => {
          props.onRun('all');
        }}
      >
        Run all{' '}
        <Kbd variant="inline" aria-hidden="true">
          ⇧⌘↵
        </Kbd>
      </Button>
      <Button
        size="sm"
        variant="destructive"
        aria-label="Cancel query"
        disabled={!active || !props.run.queryId}
        onClick={() => {
          props.runs.cancel(props.tab.id);
        }}
      >
        Cancel
      </Button>
      {props.notice ? (
        <span className={styles.notice}>{props.notice}</span>
      ) : (
        props.sessionId === null && (
          <Button size="sm" variant="ghost" onClick={props.onConnect}>
            Connect to run
          </Button>
        )
      )}
    </div>
  );
}
