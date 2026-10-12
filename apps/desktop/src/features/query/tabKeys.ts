// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import type { FormatKind } from '@openrows/editor';
import { tabCommandOf, useTabCommands } from '../commands';
import type { TabCommand } from '../commands';
import type { FileAction } from './request';
import type { RunActions } from './useRunActions';
import type { QueryRuns } from './useQueryRuns';

/** Run a tab command (shortcut, menu bar or palette) on this tab. */
function tabAction(input: {
  actions: RunActions;
  cancel: () => void;
  format: (kind: FormatKind) => void;
  file: (action: FileAction) => void;
  exportResults: () => void;
  insertSnippet: () => void;
  explain: (analyze: boolean) => void;
  assistant: () => void;
  inline: () => void;
}): (id: TabCommand) => void {
  const commands: Record<TabCommand, () => void> = {
    'run-current': () => {
      input.actions.runKey('current');
    },
    'run-all': () => {
      input.actions.runKey('all');
    },
    'run-no-limit': input.actions.runUnlimited,
    cancel: input.cancel,
    explain: () => {
      input.explain(false);
    },
    'explain-analyze': () => {
      input.explain(true);
    },
    beautify: () => {
      input.format('beautify');
    },
    uglify: () => {
      input.format('uglify');
    },
    'insert-snippet': input.insertSnippet,
    assistant: input.assistant,
    'inline-ai': input.inline,
    save: () => {
      input.file('save');
    },
    'save-as': () => {
      input.file('saveAs');
    },
    'open-file': () => {
      input.file('open');
    },
    'export-results': input.exportResults,
  };
  return (id) => {
    commands[id]();
  };
}

/** Handle a bound tab shortcut; true when the key was one. */
export function tabKey(
  event: KeyboardEvent | React.KeyboardEvent,
  act: (id: TabCommand) => void,
): boolean {
  const id = tabCommandOf(event);
  if (!id) return false;
  event.preventDefault();
  act(id);
  return true;
}

/**
 * The query tab's command handler, registered as the target of menu,
 * palette and out-of-tab shortcuts while the tab is mounted (active).
 */
export function useTabAction(input: {
  actions: RunActions;
  format: (kind: FormatKind) => void;
  files: { act: (action: FileAction) => void };
  runs: Pick<QueryRuns, 'cancel'>;
  tabId: string;
  /** Insert Snippet…: open the snippet picker. */
  onSnippet: () => void;
  onAssistant?: () => void;
  onInline?: () => void;
  /** Explain (or Explain Analyze) the statement at the cursor or the selection. */
  onExplain: (analyze: boolean) => void;
}): [
  act: (id: TabCommand) => void,
  onExportable: (exportShown: (() => void) | null) => void,
  claim: () => void,
] {
  const [exportShown, setExportShown] = useState<(() => void) | null>(null);
  const act = tabAction({
    actions: input.actions,
    format: input.format,
    file: input.files.act,
    insertSnippet: input.onSnippet,
    assistant: input.onAssistant ?? (() => undefined),
    inline: input.onInline ?? (() => undefined),
    explain: input.onExplain,
    exportResults: () => {
      exportShown?.();
    },
    cancel: () => {
      input.runs.cancel(input.tabId);
    },
  });
  const claim = useTabCommands(act);
  const onExportable = useCallback((next: (() => void) | null) => {
    setExportShown(() => next);
  }, []);
  return [act, onExportable, claim];
}
