// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import type { EditorDialect } from '@openrows/editor';
import { findParams } from '@openrows/editor/params';
import type { ParamStyleDto } from '@openrows/ipc';
import type { EditorSettings } from './editorSettings';
import { paramDtos } from './paramValues';
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
  /** Keeps the run's text on the tab. */
  tabs: Pick<QueryTabs, 'edit'>;
};

export type RunActions = {
  /** Run exactly this scope (Run's dropdown). */
  run: (scope: RunScope) => void;
  /**
   * ⌘↵ (`current`) runs the selection, else the default scope; ⇧⌘↵ (`all`)
   * runs the other scope.
   */
  runKey: (key: RunScope) => void;
  /**
   * The parameters bar's Run: resend the run that waits for values with the
   * tab's values, else run as ⌘↵ does.
   */
  provide: () => void;
};

const other = (scope: RunScope): RunScope =>
  scope === 'all' ? 'current' : 'all';

/**
 * Anonymous `?` of the document before the run's text: the bar numbers them
 * across the document, the core from the run's first statement.
 */
function anonymousShift(
  handle: EditorHandle,
  scope: RunScope,
  style: ParamStyleDto | null,
  dialect: EditorDialect,
): number {
  if (style !== 'anonymous' || scope === 'all') return 0;
  const before = handle.text().slice(0, handle.runStart());
  return findParams(before, dialect, 'anonymous').length;
}

/** Run commands of a query tab with its row limit, error policy and parameters. */
export function useRunActions(
  editor: { current: EditorHandle | null },
  settings: EditorSettings,
  input: Tab,
): RunActions {
  const { tab, runs } = input;
  const dialect = input.connection.engine;
  const style = effectiveParams(settings.queryParams, dialect);
  const shift = useRef(0);
  const params = (): ReturnType<typeof runParams> =>
    runParams(style, paramDtos(tab.params ?? {}, shift.current));
  const run = (scope: RunScope): void => {
    const handle = editor.current;
    const { sessionId } = input;
    if (!handle || !sessionId || isActive(runs.runOf(tab.id))) return;
    const request = runRequest(handle, scope, sessionId, settings.rowLimit);
    if (!request) return;
    input.tabs.edit(tab.id, handle.text());
    shift.current = anonymousShift(handle, scope, style, dialect);
    void runs.execute(tab.id, {
      ...request,
      returnOnError: settings.returnOnError,
      params: params(),
    });
  };
  const runKey = (key: RunScope): void => {
    const selected = Boolean(editor.current?.selection().trim());
    if (key === 'current') run(selected ? 'current' : settings.runDefault);
    else run(other(settings.runDefault));
  };
  return {
    run,
    runKey,
    provide: () => {
      const pending = runs.runOf(tab.id).parameters;
      if (!pending) {
        runKey('current');
        return;
      }
      void runs.execute(tab.id, { ...pending.request, params: params() });
    },
  };
}
