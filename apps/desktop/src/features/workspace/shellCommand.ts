// SPDX-License-Identifier: Apache-2.0
import type { ThemeMode } from '@openrows/ui';
import {
  isTabCommand,
  isTableCommand,
  runTabCommand,
  runTableCommand,
} from '../commands';
import type { Connections } from '../connections';
import { openLogs } from '../diagnostics';
import type { Catalog } from '../schema';
import { setGridDensity } from '../settings';
import type { SettingsSection } from '../settings';
import type { useInspectorOpen } from './layout';
import type { PaletteMode } from './WorkspacePalettes';

/** What the shell's commands act on. */
type ShellInput = {
  catalog: Catalog;
  connections: Connections;
  inspector: ReturnType<typeof useInspectorOpen>;
  toggleSidebar: () => void;
  setSidebar: (value: boolean) => void;
  setMode: (mode: PaletteMode | null) => void;
  addQuery: () => void;
  closeTab: () => void;
  /** Export the active table tab's table. */
  onExportTable: () => void;
  /** Show the ER diagram of the selected schema. */
  onShowDiagram: () => void;
  /** Open a `.sql` file into a new tab when no query tab is active. */
  openFile: () => void;
  refresh: () => void;
  toggleSafe: () => void;
  onGallery: () => void;
  onSettings: (section?: SettingsSection) => void;
  setTheme: (mode: ThemeMode) => void;
};

type Actions = Readonly<Record<string, (() => void) | undefined>>;

/** The visible schema after the selected one, wrapping around. */
function nextSchema(catalog: Catalog): string {
  const { schemas } = catalog;
  if (schemas.status !== 'ready') return catalog.schema;
  const names = schemas.data
    .filter((schema) => catalog.systemSchemas || !schema.system)
    .map((schema) => schema.name);
  return (
    names[(names.indexOf(catalog.schema) + 1) % names.length] ?? catalog.schema
  );
}

function focusSearch(input: ShellInput): void {
  input.setSidebar(true);
  requestAnimationFrame(() => {
    document.querySelector<HTMLInputElement>('[data-sidebar] input')?.focus();
  });
}

/** Panels, palettes, appearance and settings. */
function viewActions(input: ShellInput): Actions {
  const { setMode } = input;
  return {
    sidebar: input.toggleSidebar,
    inspector: input.inspector.toggle,
    database: () => {
      setMode('database');
    },
    quick: () => {
      setMode('quick');
    },
    commands: () => {
      setMode('commands');
    },
    'help-search': () => {
      setMode('commands');
    },
    search: () => {
      focusSearch(input);
    },
    schema: () => {
      input.catalog.setSchema(nextSchema(input.catalog));
    },
    gallery: input.onGallery,
    settings: () => {
      input.onSettings();
    },
    shortcuts: () => {
      input.onSettings('shortcuts');
    },
    'theme-light': () => {
      input.setTheme('light');
    },
    'theme-dark': () => {
      input.setTheme('dark');
    },
    'theme-system': () => {
      input.setTheme('system');
    },
    'grid-compact': () => {
      setGridDensity('compact');
    },
    'grid-normal': () => {
      setGridDensity('normal');
    },
    'grid-comfortable': () => {
      setGridDensity('comfortable');
    },
  };
}

/** Tabs and the active connection. */
function workActions(input: ShellInput): Actions {
  const { connections } = input;
  return {
    'new-tab': input.addQuery,
    'close-tab': input.closeTab,
    refresh: () => {
      input.refresh();
      runTableCommand('refresh');
    },
    safe: input.toggleSafe,
    'export-table': input.onExportTable,
    'show-er-diagram': input.onShowDiagram,
    'new-connection': () => {
      connections.setSheet({ kind: 'new' });
    },
    connect: connections.connect,
    disconnect: connections.disconnect,
    'open-logs': () => {
      // The failure is already in the log, and a menu command has no other place to show it.
      openLogs().catch(() => undefined);
    },
  };
}

/** A tab command no query tab took: files open anyway, a table tab exports its table. */
function tabFallback(input: ShellInput, id: string): void {
  if (id === 'open-file') input.openFile();
  if (id === 'export-results') input.onExportTable();
}

/**
 * Run a command by id; tab commands go to the active query tab, table
 * commands to the active table tab.
 */
export function shellCommand(input: ShellInput): (id: string) => void {
  return (id: string): void => {
    if (isTabCommand(id)) {
      if (!runTabCommand(id)) tabFallback(input, id);
      return;
    }
    if (isTableCommand(id) && id !== 'refresh') {
      runTableCommand(id);
      return;
    }
    (viewActions(input)[id] ?? workActions(input)[id])?.();
  };
}
