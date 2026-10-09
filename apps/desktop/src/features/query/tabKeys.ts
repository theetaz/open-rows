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
}): (id: TabCommand) => void {
  const commands: Record<TabCommand, () => void> = {
    'run-current': () => {
      input.actions.runKey('current');
    },
    'run-all': () => {
      input.actions.runKey('all');
    },
    cancel: input.cancel,
    beautify: () => {
      input.format('beautify');
    },
    uglify: () => {
      input.format('uglify');
    },
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
}): [
  act: (id: TabCommand) => void,
  onExportable: (exportShown: (() => void) | null) => void,
] {
  const [exportShown, setExportShown] = useState<(() => void) | null>(null);
  const act = tabAction({
    actions: input.actions,
    format: input.format,
    file: input.files.act,
    exportResults: () => {
      exportShown?.();
    },
    cancel: () => {
      input.runs.cancel(input.tabId);
    },
  });
  useTabCommands(act);
  const onExportable = useCallback((next: (() => void) | null) => {
    setExportShown(() => next);
  }, []);
  return [act, onExportable];
}
