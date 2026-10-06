// SPDX-License-Identifier: Apache-2.0
import type { PaletteItem } from '@openrows/ui';
/** Hard-coded commands; all actions operate on preview state only. */
export const paletteCommands: readonly PaletteItem[] = [
  { id: 'sidebar', label: 'Toggle sidebar', shortcut: '⌃⌘S' },
  { id: 'inspector', label: 'Toggle inspector', shortcut: '⌥⌘I' },
  { id: 'new-tab', label: 'New query tab', shortcut: '⌘T' },
  { id: 'close-tab', label: 'Close current tab', shortcut: '⌘W' },
  { id: 'database', label: 'Switch database…', shortcut: '⌘K' },
  { id: 'quick', label: 'Quick open…', shortcut: '⌘P' },
  { id: 'refresh', label: 'Refresh', shortcut: '—' },
  { id: 'safe', label: 'Toggle safe mode', shortcut: '—' },
  { id: 'search', label: 'Search sidebar', shortcut: '⌘F in sidebar' },
  { id: 'schema', label: 'Next schema', shortcut: '—' },
  { id: 'assistant', label: 'Show assistant', shortcut: '—' },
  { id: 'gallery', label: 'Open component gallery', shortcut: '—' },
];
