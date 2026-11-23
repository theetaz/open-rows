// SPDX-License-Identifier: Apache-2.0
import { t } from './messages';
/**
 * One command of the app. The palette, shortcuts and the native menu bar
 * (whose item ids are these ids) all run commands by id.
 */
type CommandSpec = {
  id: string;
  label: string;
  /** Default shortcut in `formatCombo` form; `null` for none. */
  shortcut: string | null;
  /** Shown instead of a shortcut when there is none (local keys). */
  hint?: string;
};

/** Commands that act on the active query tab. */
export const TAB_COMMANDS = [
  'run-current',
  'run-all',
  'run-no-limit',
  'cancel',
  'explain',
  'explain-analyze',
  'beautify',
  'uglify',
  'insert-snippet',
  'assistant',
  'inline-ai',
  'save',
  'save-as',
  'open-file',
  'export-results',
] as const;
export type TabCommand = (typeof TAB_COMMANDS)[number];

/** Commands that split the query area into two editor panes (FR-EDIT-11). */
export const PANE_COMMANDS = [
  'split-right',
  'split-down',
  'focus-pane',
  'close-split',
] as const;
export type PaneCommand = (typeof PANE_COMMANDS)[number];

/** Commands that act on the active table tab (`refresh` also reloads its rows). */
export const TABLE_COMMANDS = [
  'commit-changes',
  'discard-changes',
  'insert-row',
  'delete-row',
  'copy-selection',
  'copy-as-csv',
  'copy-as-json',
  'copy-as-markdown',
  'copy-as-insert',
  'copy-as-where-in',
  'copy-with-headers',
  'pin-column',
  'reset-columns',
  'focus-filter',
  'toggle-columns',
  'toggle-row-form',
  'open-foreign-key',
  'refresh',
] as const;
export type TableCommand = (typeof TABLE_COMMANDS)[number];

/** Every command, in palette order. */
export const COMMANDS: readonly CommandSpec[] = [
  { id: 'lock-app', label: t('lockOpenrows'), shortcut: 'Ctrl+Cmd+L' },
  { id: 'switch-connection', label: t('switchConnection'), shortcut: null },
  { id: 'reopen-session', label: t('reopenPreviousSession'), shortcut: null },
  { id: 'sidebar', label: t('toggleSidebar'), shortcut: 'Ctrl+Cmd+S' },
  { id: 'inspector', label: t('toggleInspector'), shortcut: 'Alt+Cmd+I' },
  { id: 'new-tab', label: t('newQueryTab'), shortcut: 'Cmd+T' },
  { id: 'new-window', label: t('newWindow'), shortcut: 'Shift+Cmd+N' },
  {
    id: 'move-tab-to-new-window',
    label: t('moveTabNew'),
    shortcut: null,
  },
  { id: 'close-tab', label: t('closeCurrentTab'), shortcut: 'Cmd+W' },
  {
    id: 'close-other-tabs',
    label: t('closeOtherTabs'),
    shortcut: 'Alt+Cmd+W',
  },
  {
    id: 'next-tab',
    label: t('selectNextTab'),
    shortcut: 'Shift+Cmd+BracketRight',
  },
  {
    id: 'previous-tab',
    label: t('selectPreviousTab'),
    shortcut: 'Shift+Cmd+BracketLeft',
  },
  { id: 'duplicate-tab', label: t('duplicateTab'), shortcut: 'Shift+Cmd+K' },
  { id: 'pin-tab', label: t('pinTab'), shortcut: null },
  { id: 'go-back', label: t('goBack'), shortcut: 'Ctrl+Cmd+ArrowLeft' },
  { id: 'go-forward', label: t('goForward'), shortcut: 'Ctrl+Cmd+ArrowRight' },
  { id: 'open-location', label: t('openLocation'), shortcut: 'Cmd+L' },
  { id: 'database', label: t('openDatabase'), shortcut: 'Cmd+K' },
  {
    id: 'toggle-favourite',
    label: t('toggleObjectFavourite'),
    shortcut: 'Alt+Cmd+D',
  },
  { id: 'quick', label: t('quickOpen'), shortcut: 'Cmd+P' },
  { id: 'commands', label: t('commandPalette'), shortcut: 'Shift+Cmd+P' },
  { id: 'refresh', label: t('refresh'), shortcut: 'Cmd+R' },
  { id: 'run-current', label: t('runCurrentStatement'), shortcut: 'Cmd+Enter' },
  { id: 'run-all', label: t('runAllStatements'), shortcut: 'Shift+Cmd+Enter' },
  {
    id: 'run-no-limit',
    label: t('runWithoutAutomatic'),
    shortcut: 'Alt+Cmd+Enter',
  },
  { id: 'cancel', label: t('cancelQuery'), shortcut: 'Cmd+Period' },
  { id: 'explain', label: t('explainQueryPlan'), shortcut: 'Alt+Cmd+E' },
  {
    id: 'explain-analyze',
    label: t('explainAnalyzeRuns'),
    shortcut: 'Alt+Shift+Cmd+E',
  },
  { id: 'beautify', label: t('beautifySql'), shortcut: 'Alt+Cmd+B' },
  { id: 'uglify', label: t('uglifySql'), shortcut: 'Shift+Cmd+I' },
  { id: 'insert-snippet', label: t('insertSnippet'), shortcut: null },
  { id: 'inline-ai', label: t('editSqlAi'), shortcut: 'Cmd+I' },
  { id: 'assistant', label: t('toggleQueryAssistant'), shortcut: null },
  { id: 'open-file', label: t('openSqlFile'), shortcut: 'Cmd+O' },
  { id: 'run-sql-file', label: t('runSqlFile'), shortcut: null },
  { id: 'save', label: t('saveSqlFile'), shortcut: 'Cmd+S' },
  { id: 'save-as', label: t('saveSqlFile2'), shortcut: 'Shift+Cmd+S' },
  { id: 'open-project', label: t('openProject'), shortcut: null },
  { id: 'close-project', label: t('closeProject'), shortcut: null },
  {
    id: 'split-right',
    label: t('splitEditorRight'),
    shortcut: 'Cmd+Backslash',
  },
  {
    id: 'split-down',
    label: t('splitEditorDown'),
    shortcut: 'Shift+Cmd+Backslash',
  },
  {
    id: 'focus-pane',
    label: t('focusOtherEditor'),
    shortcut: 'Alt+Cmd+Backslash',
  },
  { id: 'close-split', label: t('closeSplitPane'), shortcut: null },
  { id: 'export-table', label: t('exportTable'), shortcut: null },
  { id: 'import-data', label: t('importDataFile'), shortcut: null },
  { id: 'export-tables', label: t('exportTablesSchema'), shortcut: null },
  { id: 'backup-database', label: t('backUpDatabase'), shortcut: null },
  { id: 'backup-schema', label: t('backUpSchema'), shortcut: null },
  { id: 'backup-table', label: t('backUpTable'), shortcut: null },
  { id: 'restore-schema', label: t('restoreSchema'), shortcut: null },
  { id: 'restore-database', label: t('restoreDatabase'), shortcut: null },
  { id: 'show-er-diagram', label: t('showErDiagram'), shortcut: null },
  { id: 'show-jobs', label: t('showBackgroundJobs'), shortcut: null },
  {
    id: 'export-results',
    label: t('exportResults'),
    shortcut: 'Shift+Cmd+E',
  },
  { id: 'commit-changes', label: t('commitChanges'), shortcut: 'Alt+Cmd+S' },
  { id: 'discard-changes', label: t('discardChanges'), shortcut: null },
  { id: 'insert-row', label: t('insertRow'), shortcut: 'Alt+Cmd+N' },
  {
    id: 'delete-row',
    label: t('deleteSelectedRows'),
    shortcut: 'Alt+Cmd+Backspace',
  },
  {
    id: 'copy-selection',
    label: t('copySelectedRows'),
    shortcut: 'Alt+Cmd+C',
  },
  { id: 'copy-as-csv', label: t('copySelectionCsv'), shortcut: null },
  { id: 'copy-as-json', label: t('copySelectionJson'), shortcut: null },
  {
    id: 'copy-as-markdown',
    label: t('copySelectionMarkdown'),
    shortcut: null,
  },
  {
    id: 'copy-as-insert',
    label: t('copySelectionSql'),
    shortcut: null,
  },
  {
    id: 'copy-as-where-in',
    label: t('copySelectionWhere'),
    shortcut: null,
  },
  {
    id: 'copy-with-headers',
    label: t('copySelectionColumn'),
    shortcut: 'Alt+Shift+Cmd+C',
  },
  { id: 'pin-column', label: t('pinUnpinColumn'), shortcut: null },
  { id: 'reset-columns', label: t('resetColumnLayout'), shortcut: null },
  {
    id: 'focus-filter',
    label: t('filterRows'),
    shortcut: null,
    hint: t('fTable'),
  },
  {
    id: 'toggle-columns',
    label: t('showHideColumns'),
    shortcut: 'Alt+Cmd+K',
  },
  {
    id: 'toggle-row-form',
    label: t('showRowForm'),
    shortcut: 'Alt+Cmd+F',
  },
  {
    id: 'open-foreign-key',
    label: t('openReferencedRow'),
    shortcut: null,
  },
  { id: 'export-connections', label: t('exportConnections'), shortcut: null },
  { id: 'import-connections', label: t('importConnections'), shortcut: null },
  { id: 'new-connection', label: t('newConnection'), shortcut: null },
  { id: 'connect', label: t('connect'), shortcut: null },
  { id: 'disconnect', label: t('disconnect'), shortcut: null },
  { id: 'safe', label: t('safeModeSettings'), shortcut: null },
  {
    id: 'search',
    label: t('searchSidebar'),
    shortcut: null,
    hint: t('fSidebar'),
  },
  { id: 'schema', label: t('nextSchema'), shortcut: null },
  { id: 'monitor', label: t('showMonitor'), shortcut: null },
  { id: 'processes', label: t('showProcesses'), shortcut: null },
  { id: 'new-table', label: t('newTable'), shortcut: null },
  { id: 'new-view', label: t('newView'), shortcut: null },
  {
    id: 'new-materialized-view',
    label: t('newMaterializedView'),
    shortcut: null,
  },
  { id: 'new-function', label: t('newFunction'), shortcut: null },
  { id: 'new-procedure', label: t('newProcedure'), shortcut: null },
  { id: 'new-trigger', label: t('newTriggerTable'), shortcut: null },
  { id: 'rename-table', label: t('renameTable'), shortcut: null },
  { id: 'duplicate-table', label: t('duplicateTable'), shortcut: null },
  { id: 'truncate-table', label: t('truncateTable'), shortcut: null },
  { id: 'drop-table', label: t('dropTable'), shortcut: null },
  { id: 'edit-view', label: t('editView'), shortcut: null },
  { id: 'drop-view', label: t('dropView'), shortcut: null },
  { id: 'new-database', label: t('newDatabase'), shortcut: null },
  { id: 'rename-database', label: t('renameDatabase'), shortcut: null },
  { id: 'drop-database', label: t('dropDatabase'), shortcut: null },
  { id: 'new-schema', label: t('newSchema'), shortcut: null },
  { id: 'rename-schema', label: t('renameSchema'), shortcut: null },
  { id: 'drop-schema', label: t('dropSchema'), shortcut: null },
  { id: 'settings', label: t('settings'), shortcut: 'Cmd+Comma' },
  { id: 'shortcuts', label: t('keyboardShortcuts'), shortcut: null },
  { id: 'help-search', label: t('searchCommands'), shortcut: null },
  { id: 'open-logs', label: t('openLogsFolder'), shortcut: null },
  { id: 'theme-light', label: t('appearanceLight'), shortcut: null },
  { id: 'theme-dark', label: t('appearanceDark'), shortcut: null },
  { id: 'theme-system', label: t('appearanceSystem'), shortcut: null },
  { id: 'grid-compact', label: t('gridRowsCompact'), shortcut: null },
  { id: 'grid-normal', label: t('gridRowsNormal'), shortcut: null },
  { id: 'grid-comfortable', label: t('gridRowsComfortable'), shortcut: null },
  { id: 'gallery', label: t('openComponentGallery'), shortcut: null },
];

/** Shortcuts the system or the Edit and Window menus own. */
export const RESERVED: Readonly<Record<string, string>> = {
  'Cmd+Q': t('quit'),
  'Cmd+H': t('hide'),
  'Alt+Cmd+H': t('hideOthers'),
  'Cmd+M': t('minimize'),
  'Cmd+Z': t('undo'),
  'Shift+Cmd+Z': t('redo'),
  'Cmd+X': t('cut'),
  'Cmd+C': t('copy'),
  'Cmd+V': t('paste'),
  'Cmd+A': t('selectAll'),
  'Ctrl+Cmd+F': t('enterFullScreen'),
  'Cmd+F': t('find'),
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [
      `Cmd+${String(index + 1)}`,
      t('selectTabNumber'),
    ]),
  ),
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [
      `Alt+Cmd+${String(index + 1)}`,
      t('switchConnection2'),
    ]),
  ),
};

/** The spec of a command id. */
export function commandSpec(id: string): CommandSpec | undefined {
  return COMMANDS.find((command) => command.id === id);
}

export function isTabCommand(id: string): id is TabCommand {
  return (TAB_COMMANDS as readonly string[]).includes(id);
}

export function isPaneCommand(id: string): id is PaneCommand {
  return (PANE_COMMANDS as readonly string[]).includes(id);
}

export function isTableCommand(id: string): id is TableCommand {
  return (TABLE_COMMANDS as readonly string[]).includes(id);
}
