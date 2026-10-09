// SPDX-License-Identifier: Apache-2.0
import type { EditorDialect } from '@openrows/editor';
import type { EditorSettings } from './editorSettings';
import { effectiveParams, runParams } from './queryParams';
import { runRequest } from './request';
import type { EditorHandle, RunScope } from './request';
import { isActive } from './run';
import type { QueryTab } from './tabs';
import type { QueryRuns } from './useQueryRuns';
import type { QueryTabs } from './useQueryTabs';

/** The query tab's props the run commands use. */
type Tab = {
  tab: QueryTab;
  sessionId: string | null;
  connection: { engine: EditorDialect };
  runs: QueryRuns;
  /** Keeps the run's text and parameter values on the tab. */
  tabs: Pick<QueryTabs, 'edit' | 'setParams'>;
};

export type RunActions = {
  /** Run exactly this scope (Run's dropdown). */
  run: (scope: RunScope) => void;
  /**
   * ⌘↵ (`current`) runs the selection, else the default scope; ⇧⌘↵ (`all`)
   * runs the other scope.
   */
  runKey: (key: RunScope) => void;
  /** Resend the run that asked for parameters with these values. */
  provide: (values: Readonly<Record<string, string>>) => void;
};

const other = (scope: RunScope): RunScope =>
  scope === 'all' ? 'current' : 'all';

/** Run commands of a query tab with its row limit, error policy and parameters. */
export function useRunActions(
  editor: { current: EditorHandle | null },
  settings: EditorSettings,
  input: Tab,
): RunActions {
  const { tab, runs } = input;
  const style = effectiveParams(settings.queryParams, input.connection.engine);
  const run = (scope: RunScope): void => {
    const handle = editor.current;
    const { sessionId } = input;
    if (!handle || !sessionId || isActive(runs.runOf(tab.id))) return;
    const request = runRequest(handle, scope, sessionId, settings.rowLimit);
    if (!request) return;
    input.tabs.edit(tab.id, handle.text());
    void runs.execute(tab.id, {
      ...request,
      returnOnError: settings.returnOnError,
      params: runParams(style, null),
    });
  };
  return {
    run,
    runKey: (key) => {
      const selected = Boolean(editor.current?.selection().trim());
      if (key === 'current') run(selected ? 'current' : settings.runDefault);
      else run(other(settings.runDefault));
    },
    provide: (values) => {
      const pending = runs.runOf(tab.id).parameters;
      if (!pending) return;
      input.tabs.setParams(tab.id, values);
      void runs.execute(tab.id, {
        ...pending.request,
        params: runParams(style, values),
      });
    },
  };
}
