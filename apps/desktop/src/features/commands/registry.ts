// SPDX-License-Identifier: Apache-2.0
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
  { id: 'lock-app', label: 'Lock OpenRows', shortcut: 'Ctrl+Cmd+L' },
  { id: 'switch-connection', label: 'Switch Connection…', shortcut: null },
  { id: 'reopen-session', label: 'Reopen Previous Session', shortcut: null },
  { id: 'sidebar', label: 'Toggle sidebar', shortcut: 'Ctrl+Cmd+S' },
  { id: 'inspector', label: 'Toggle inspector', shortcut: 'Alt+Cmd+I' },
  { id: 'new-tab', label: 'New query tab', shortcut: 'Cmd+T' },
  { id: 'close-tab', label: 'Close current tab', shortcut: 'Cmd+W' },
  { id: 'database', label: 'Open database…', shortcut: 'Cmd+K' },
  {
    id: 'toggle-favourite',
    label: 'Toggle object favourite',
    shortcut: 'Alt+Cmd+D',
  },
  { id: 'quick', label: 'Quick open…', shortcut: 'Cmd+P' },
  { id: 'commands', label: 'Command palette', shortcut: 'Shift+Cmd+P' },
  { id: 'refresh', label: 'Refresh', shortcut: 'Cmd+R' },
  { id: 'run-current', label: 'Run current statement', shortcut: 'Cmd+Enter' },
  { id: 'run-all', label: 'Run all statements', shortcut: 'Shift+Cmd+Enter' },
  {
    id: 'run-no-limit',
    label: 'Run without automatic LIMIT',
    shortcut: 'Alt+Cmd+Enter',
  },
  { id: 'cancel', label: 'Cancel query', shortcut: 'Cmd+Period' },
  { id: 'explain', label: 'Explain query plan', shortcut: 'Alt+Cmd+E' },
  {
    id: 'explain-analyze',
    label: 'Explain analyze (runs the statement)',
    shortcut: 'Alt+Shift+Cmd+E',
  },
  { id: 'beautify', label: 'Beautify SQL', shortcut: 'Cmd+I' },
  { id: 'uglify', label: 'Uglify SQL', shortcut: 'Shift+Cmd+I' },
  { id: 'insert-snippet', label: 'Insert snippet…', shortcut: null },
  { id: 'assistant', label: 'Toggle query assistant', shortcut: null },
  { id: 'open-file', label: 'Open SQL file…', shortcut: 'Cmd+O' },
  { id: 'run-sql-file', label: 'Run SQL file…', shortcut: null },
  { id: 'save', label: 'Save SQL file', shortcut: 'Cmd+S' },
  { id: 'save-as', label: 'Save SQL file as…', shortcut: 'Shift+Cmd+S' },
  { id: 'open-project', label: 'Open project…', shortcut: null },
  { id: 'close-project', label: 'Close project', shortcut: null },
  { id: 'split-right', label: 'Split editor right', shortcut: 'Cmd+Backslash' },
  {
    id: 'split-down',
    label: 'Split editor down',
    shortcut: 'Shift+Cmd+Backslash',
  },
  {
    id: 'focus-pane',
    label: 'Focus other editor pane',
    shortcut: 'Alt+Cmd+Backslash',
  },
  { id: 'close-split', label: 'Close split pane', shortcut: null },
  { id: 'export-table', label: 'Export table…', shortcut: null },
  { id: 'import-data', label: 'Import data from file…', shortcut: null },
  { id: 'export-tables', label: 'Export tables of schema…', shortcut: null },
  { id: 'backup-database', label: 'Back up database…', shortcut: null },
  { id: 'backup-schema', label: 'Back up schema…', shortcut: null },
  { id: 'backup-table', label: 'Back up table…', shortcut: null },
  { id: 'restore-schema', label: 'Restore schema…', shortcut: null },
  { id: 'restore-database', label: 'Restore database…', shortcut: null },
  { id: 'show-er-diagram', label: 'Show ER diagram', shortcut: null },
  { id: 'show-jobs', label: 'Show background jobs', shortcut: null },
  {
    id: 'export-results',
    label: 'Export results…',
    shortcut: 'Shift+Cmd+E',
  },
  { id: 'commit-changes', label: 'Commit changes…', shortcut: 'Alt+Cmd+S' },
  { id: 'discard-changes', label: 'Discard changes', shortcut: null },
  { id: 'insert-row', label: 'Insert row', shortcut: 'Alt+Cmd+N' },
  {
    id: 'delete-row',
    label: 'Delete selected rows',
    shortcut: 'Alt+Cmd+Backspace',
  },
  {
    id: 'copy-selection',
    label: 'Copy selected rows or cells',
    shortcut: 'Alt+Cmd+C',
  },
  { id: 'copy-as-csv', label: 'Copy selection as CSV', shortcut: null },
  { id: 'copy-as-json', label: 'Copy selection as JSON', shortcut: null },
  {
    id: 'copy-as-markdown',
    label: 'Copy selection as Markdown',
    shortcut: null,
  },
  {
    id: 'copy-as-insert',
    label: 'Copy selection as SQL INSERT',
    shortcut: null,
  },
  {
    id: 'copy-as-where-in',
    label: 'Copy selection as WHERE IN',
    shortcut: null,
  },
  {
    id: 'copy-with-headers',
    label: 'Copy selection with column headers',
    shortcut: 'Alt+Shift+Cmd+C',
  },
  { id: 'pin-column', label: 'Pin or unpin column', shortcut: null },
  { id: 'reset-columns', label: 'Reset column layout', shortcut: null },
  {
    id: 'focus-filter',
    label: 'Filter rows',
    shortcut: null,
    hint: '⌘F in a table',
  },
  {
    id: 'toggle-columns',
    label: 'Show or hide columns panel',
    shortcut: 'Alt+Cmd+K',
  },
  {
    id: 'toggle-row-form',
    label: 'Show row as form or grid',
    shortcut: 'Alt+Cmd+F',
  },
  {
    id: 'open-foreign-key',
    label: 'Open referenced row',
    shortcut: null,
  },
  { id: 'export-connections', label: 'Export connections…', shortcut: null },
  { id: 'import-connections', label: 'Import connections…', shortcut: null },
  { id: 'new-connection', label: 'New connection…', shortcut: null },
  { id: 'connect', label: 'Connect', shortcut: null },
  { id: 'disconnect', label: 'Disconnect', shortcut: null },
  { id: 'safe', label: 'Toggle safe mode', shortcut: null },
  {
    id: 'search',
    label: 'Search sidebar',
    shortcut: null,
    hint: '⌘F in sidebar',
  },
  { id: 'schema', label: 'Next schema', shortcut: null },
  { id: 'monitor', label: 'Show Monitor', shortcut: null },
  { id: 'processes', label: 'Show processes', shortcut: null },
  { id: 'new-table', label: 'New table…', shortcut: null },
  { id: 'new-view', label: 'New view…', shortcut: null },
  {
    id: 'new-materialized-view',
    label: 'New materialized view…',
    shortcut: null,
  },
  { id: 'new-function', label: 'New function…', shortcut: null },
  { id: 'new-procedure', label: 'New procedure…', shortcut: null },
  { id: 'new-trigger', label: 'New trigger on table…', shortcut: null },
  { id: 'rename-table', label: 'Rename table…', shortcut: null },
  { id: 'duplicate-table', label: 'Duplicate table…', shortcut: null },
  { id: 'truncate-table', label: 'Truncate table…', shortcut: null },
  { id: 'drop-table', label: 'Drop table…', shortcut: null },
  { id: 'edit-view', label: 'Edit view…', shortcut: null },
  { id: 'drop-view', label: 'Drop view…', shortcut: null },
  { id: 'new-database', label: 'New database…', shortcut: null },
  { id: 'rename-database', label: 'Rename database…', shortcut: null },
  { id: 'drop-database', label: 'Drop database…', shortcut: null },
  { id: 'new-schema', label: 'New schema…', shortcut: null },
  { id: 'rename-schema', label: 'Rename schema…', shortcut: null },
  { id: 'drop-schema', label: 'Drop schema…', shortcut: null },
  { id: 'settings', label: 'Settings…', shortcut: 'Cmd+Comma' },
  { id: 'shortcuts', label: 'Keyboard shortcuts…', shortcut: null },
  { id: 'help-search', label: 'Search commands…', shortcut: null },
  { id: 'open-logs', label: 'Open logs folder', shortcut: null },
  { id: 'theme-light', label: 'Appearance: light', shortcut: null },
  { id: 'theme-dark', label: 'Appearance: dark', shortcut: null },
  { id: 'theme-system', label: 'Appearance: system', shortcut: null },
  { id: 'grid-compact', label: 'Grid rows: compact', shortcut: null },
  { id: 'grid-normal', label: 'Grid rows: normal', shortcut: null },
  { id: 'grid-comfortable', label: 'Grid rows: comfortable', shortcut: null },
  { id: 'gallery', label: 'Open component gallery', shortcut: null },
];

/** Shortcuts the system or the Edit and Window menus own. */
export const RESERVED: Readonly<Record<string, string>> = {
  'Cmd+Q': 'Quit',
  'Cmd+H': 'Hide',
  'Alt+Cmd+H': 'Hide Others',
  'Cmd+M': 'Minimize',
  'Cmd+Z': 'Undo',
  'Shift+Cmd+Z': 'Redo',
  'Cmd+X': 'Cut',
  'Cmd+C': 'Copy',
  'Cmd+V': 'Paste',
  'Cmd+A': 'Select All',
  'Ctrl+Cmd+F': 'Enter Full Screen',
  'Cmd+F': 'Find',
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [
      `Cmd+${String(index + 1)}`,
      'Switch connection',
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
