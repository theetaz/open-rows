// SPDX-License-Identifier: Apache-2.0
import type { MenuEntry } from '../../shell/CascadeMenu';
import type { RunScope } from './request';
import { t } from './lazyMessages';

/** What Run's dropdown can do in the tab right now. */
export type RunMenu = {
  /** A session is open and nothing runs. */
  ready: boolean;
  /** What the Run button and ⌘↵ run. */
  runDefault: RunScope;
  onRun: (scope: RunScope) => void;
  /** Run as ⌘↵ does, once without the automatic `LIMIT`. */
  onRunUnlimited: () => void;
  onDefault: (scope: RunScope) => void;
  onSaveAs: () => void;
  onOpenFile: () => void;
  onAddToQueries: () => void;
};

const LABELS: Record<RunScope, string> = {
  current: t('runCurrent'),
  all: t('runAll'),
};

/** `Run Current` or `Run All`. */
export function runLabel(scope: RunScope): string {
  return LABELS[scope];
}

/** ⌘↵ runs the default, ⇧⌘↵ the other scope. */
function runShortcut(scope: RunScope, runDefault: RunScope): string {
  return scope === runDefault ? '⌘↵' : '⇧⌘↵';
}

const item = (
  label: string,
  run: () => void,
  options: { shortcut?: string; disabled?: boolean } = {},
): MenuEntry => ({ kind: 'item', label, run, ...options });

/** Explain Analyze, which runs the statement, where the engine can explain. */
function analyzeEntries(
  menu: RunMenu,
  explain: ((analyze: boolean) => void) | undefined,
): MenuEntry[] {
  if (!explain) return [];
  const run = (): void => {
    explain(true);
  };
  return [
    item(t('explainAnalyze'), run, {
      shortcut: '⌥⇧⌘E',
      disabled: !menu.ready,
    }),
  ];
}

/**
 * Run's dropdown: run commands, Explain Analyze, the default for ⌘↵, then
 * file and saved-query actions.
 */
export function runEntries(
  menu: RunMenu,
  explain?: (analyze: boolean) => void,
): MenuEntry[] {
  const scopes: RunScope[] = ['all', 'current'];
  return [
    ...scopes.map((scope) =>
      item(
        LABELS[scope],
        () => {
          menu.onRun(scope);
        },
        {
          shortcut: runShortcut(scope, menu.runDefault),
          disabled: !menu.ready,
        },
      ),
    ),
    item(t('runWithoutLimit'), menu.onRunUnlimited, {
      shortcut: '⌥⌘↵',
      disabled: !menu.ready,
    }),
    ...analyzeEntries(menu, explain),
    {
      kind: 'submenu',
      label: t('default'),
      entries: scopes.map((scope) => ({
        kind: 'item',
        label: LABELS[scope],
        checked: menu.runDefault === scope,
        radio: true,
        run: () => {
          menu.onDefault(scope);
        },
      })),
    },
    { kind: 'separator' },
    item(t('saveSql'), menu.onSaveAs, { shortcut: '⇧⌘S' }),
    item(t('openSqlFile'), menu.onOpenFile, { shortcut: '⌘O' }),
    item(t('addQueries2'), menu.onAddToQueries),
  ];
}
