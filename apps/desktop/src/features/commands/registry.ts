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
  'cancel',
  'beautify',
  'uglify',
  'insert-snippet',
  'save',
  'save-as',
  'open-file',
  'export-results',
] as const;
export type TabCommand = (typeof TAB_COMMANDS)[number];

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
  'pin-column',
  'reset-columns',
  'focus-filter',
  'toggle-columns',
  'refresh',
] as const;
export type TableCommand = (typeof TABLE_COMMANDS)[number];

/** Every command, in palette order. */
export const COMMANDS: readonly CommandSpec[] = [
  { id: 'sidebar', label: 'Toggle sidebar', shortcut: 'Ctrl+Cmd+S' },
  { id: 'inspector', label: 'Toggle inspector', shortcut: 'Alt+Cmd+I' },
  { id: 'new-tab', label: 'New query tab', shortcut: 'Cmd+T' },
  { id: 'close-tab', label: 'Close current tab', shortcut: 'Cmd+W' },
  { id: 'database', label: 'Open database…', shortcut: 'Cmd+K' },
  { id: 'quick', label: 'Quick open…', shortcut: 'Cmd+P' },
  { id: 'commands', label: 'Command palette', shortcut: 'Shift+Cmd+P' },
  { id: 'refresh', label: 'Refresh', shortcut: 'Cmd+R' },
  { id: 'run-current', label: 'Run current statement', shortcut: 'Cmd+Enter' },
  { id: 'run-all', label: 'Run all statements', shortcut: 'Shift+Cmd+Enter' },
  { id: 'cancel', label: 'Cancel query', shortcut: 'Cmd+Period' },
  { id: 'beautify', label: 'Beautify SQL', shortcut: 'Cmd+I' },
  { id: 'uglify', label: 'Uglify SQL', shortcut: 'Shift+Cmd+I' },
  { id: 'insert-snippet', label: 'Insert snippet…', shortcut: null },
  { id: 'open-file', label: 'Open SQL file…', shortcut: 'Cmd+O' },
  { id: 'save', label: 'Save SQL file', shortcut: 'Cmd+S' },
  { id: 'save-as', label: 'Save SQL file as…', shortcut: 'Shift+Cmd+S' },
  { id: 'export-table', label: 'Export table…', shortcut: null },
  { id: 'show-er-diagram', label: 'Show ER diagram', shortcut: null },
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
  { id: 'processes', label: 'Show processes', shortcut: null },
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

export function isTableCommand(id: string): id is TableCommand {
  return (TABLE_COMMANDS as readonly string[]).includes(id);
}
