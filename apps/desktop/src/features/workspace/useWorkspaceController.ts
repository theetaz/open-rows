// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { openSqlFile } from '@openrows/ipc';
import { useTheme } from '@openrows/ui';
import type { Connections } from '../connections';
import { useCatalog } from '../schema';
import type { Catalog, ObjectRef } from '../schema';
import { tabFile, useQueryTabs } from '../query';
import type { QueryTabs, TabScope } from '../query';
import { openTableView } from '../table';
import { useMenuBridge } from '../commands';
import type { SettingsSection } from '../settings';
import { useDetails } from './useDetails';
import type { Details } from './useDetails';
import { useTableInfo } from './useTableInfo';
import type { TableInfoState } from './useTableInfo';
import { queryCommands } from './queryCommands';
import { useCompact, useInspectorOpen } from './layout';
import { useWorkspaceKeys } from './useWorkspaceKeys';
import { useDatabasePrompt } from './useDatabasePrompt';
import { shellCommand } from './shellCommand';
import { menuEnabled } from './menuState';
import type { PaletteMode } from './WorkspacePalettes';
import { importOpener } from './TableImport';
import type { ImportTarget } from './TableImport';
/**
 * Tab scopes of the active connection: the active database's scope (the
 * database last chosen, else the profile's, until one is open, so restored
 * tabs show before reconnecting; none without either) and a mapper for any
 * database.
 */
function scopes(connections: Connections): {
  scope: TabScope | undefined;
  scopeOf: (database: string) => TabScope | undefined;
} {
  const profile = connections.active;
  const scopeOf = (database: string): TabScope | undefined =>
    profile && database ? { connectionId: profile.id, database } : undefined;
  const database =
    connections.database || connections.wanted || (profile?.database ?? '');
  return { scope: scopeOf(database), scopeOf };
}

/** Open a `.sql` file into a new tab of `scope` (no query tab is active). */
function openFileInto(tabs: QueryTabs, scope: TabScope | undefined): void {
  if (!scope) return;
  openSqlFile().then(
    (opened) => {
      if (!opened) return;
      const { file, sql } = opened;
      tabs.open(scope, {
        title: file.name,
        sql,
        file: tabFile(file, sql, file.modifiedMs),
      });
    },
    () => undefined,
  );
}

/** Catalog refreshes, counted for the screen-reader announcement. */
function useRefresh(catalog: Catalog): [number, () => void] {
  const [refreshes, setRefreshes] = useState(0);
  return [
    refreshes,
    () => {
      catalog.refresh();
      setRefreshes((count) => count + 1);
    },
  ];
}

/** Opens (or refocuses) a scope's processes tab (FR-ADMIN-01); the active scope by default. */
function processesOpener(
  connections: Connections,
  tabs: QueryTabs,
  scope: TabScope | undefined,
): (target?: TabScope) => void {
  return (target = scope) => {
    const profile = connections.profiles.find(
      (item) => item.id === target?.connectionId,
    );
    if (target && profile?.processList)
      tabs.open(target, { processes: true, title: 'Processes' });
  };
}

/** Shortcuts and the native menu bar, both routed to `command`. */
function useCommandRoutes(
  command: (id: string) => void,
  enabled: boolean,
  state: Parameters<typeof menuEnabled>[0],
): void {
  useWorkspaceKeys({
    enabled,
    command,
    connection: state.connections.selectIndex,
  });
  useMenuBridge(command, menuEnabled(state));
}

/** The schema an ER diagram shows, and the table to centre on. */
export type DiagramRequest = {
  database: string;
  schema: string;
  focus?: string;
};

/** The diagram of a tree object's schema, else of the selected schema. */
function diagramOpener(
  catalog: Catalog,
  show: (request: DiagramRequest) => void,
): (object?: ObjectRef) => void {
  return (object) => {
    const database = object?.database ?? catalog.database;
    const schema = object?.schema ?? catalog.schema;
    if (!database || !schema) return;
    show({ database, schema, ...(object ? { focus: object.name } : {}) });
  };
}

type Input = {
  onGallery: () => void;
  onSettings: (section?: SettingsSection) => void;
  onExportTable: () => void;
  /** Open the import sheet for a database, schema and maybe a table. */
  onImport: (target: ImportTarget) => void;
  onShowDiagram: (request: DiagramRequest) => void;
  enabled: boolean;
  connections: Connections;
};

/** The toolbar's safe-mode switch; production connections are always safe. */
function useSafeMode(prod: boolean): [boolean, () => void] {
  const [safe, setSafe] = useState(false);
  return [
    safe || prod,
    () => {
      if (!prod) setSafe(!safe);
    },
  ];
}

/** Relations open as table views (D-08); routines and sequences as query tabs. */
function objectOpener(input: {
  catalog: Catalog;
  tabs: QueryTabs;
  scopeOf: (database: string) => TabScope | undefined;
  commands: ReturnType<typeof queryCommands>;
}): (object: ObjectRef, preview?: boolean) => void {
  const { catalog, tabs, scopeOf, commands } = input;
  return (object, preview = false) => {
    catalog.setSchema(object.schema);
    const target = scopeOf(object.database);
    if (!target || !openTableView(tabs, target, object, preview))
      commands.openObject(object);
  };
}

/** Shell state and command wiring, isolated from rendering. */
export function useWorkspaceController(input: Input): WorkspaceController {
  const { connections } = input;
  const catalog = useCatalog(connections);
  const tabs = useQueryTabs();
  const { setMode: setTheme } = useTheme();
  const { scope, scopeOf } = scopes(connections);
  const compact = useCompact();
  const inspector = useInspectorOpen(compact);
  const [sidebar, setSidebar] = useState(true);
  const [mode, setMode] = useState<PaletteMode | null>(null);
  useDatabasePrompt(connections, setMode);
  const [refreshes, refresh] = useRefresh(catalog);
  const active = scope ? tabs.activeFor(scope) : undefined;
  const details = useDetails(active?.id);
  const tableInfo = useTableInfo(active?.id);
  const commands = queryCommands(tabs, scope, scopeOf);
  const openProcesses = processesOpener(connections, tabs, scope);
  const openObject = objectOpener({ catalog, tabs, scopeOf, commands });
  const prod = connections.active?.env.kind === 'prod';
  const [safe, toggleSafe] = useSafeMode(prod);
  const toggleSidebar = (): void => {
    setSidebar((open) => !open);
  };
  const showDiagram = diagramOpener(catalog, input.onShowDiagram);
  const tableTab = active?.kind === 'table' ? active.table : undefined;
  const importData = importOpener(catalog, tableTab, input.onImport);
  const command = shellCommand({
    ...input,
    catalog,
    inspector,
    toggleSidebar,
    setSidebar,
    setMode,
    addQuery: commands.addQuery,
    closeTab: commands.closeTab,
    openFile: () => {
      openFileInto(tabs, scope);
    },
    refresh,
    toggleSafe,
    setTheme,
    onShowDiagram: () => {
      showDiagram();
    },
    onImport: importData,
    // Called without arguments: the active scope.
    openProcesses,
  });
  useCommandRoutes(command, input.enabled && !mode, {
    connections,
    tab: active,
    scope,
    prod,
  });
  return {
    catalog,
    tabs,
    scope,
    compact,
    inspector,
    sidebar,
    safe,
    prod,
    mode,
    setMode,
    refreshes,
    ...details,
    ...tableInfo,
    toggleSidebar,
    addQuery: commands.addQuery,
    openObject,
    showDiagram,
    importData,
    openProcesses,
    hiddenCommands: connections.active?.processList ? [] : ['processes'],
    toggleSafe,
    refresh,
    command,
  };
}

type WorkspaceController = Details &
  TableInfoState & {
    catalog: Catalog;
    tabs: QueryTabs;
    /** Tab scope of the browsed connection and database. */
    scope: TabScope | undefined;
    compact: boolean;
    inspector: ReturnType<typeof useInspectorOpen>;
    sidebar: boolean;
    safe: boolean;
    prod: boolean;
    mode: PaletteMode | null;
    setMode: (mode: PaletteMode | null) => void;
    refreshes: number;
    toggleSidebar: () => void;
    /** Open a query tab on the active connection, optionally with SQL. */
    addQuery: (sql?: string) => void;
    /**
     * Open (or refocus) the table view or query tab of a tree or ⌘P object;
     * `preview` (a single click in the tree) reuses the preview tab.
     */
    openObject: (object: ObjectRef, preview?: boolean) => void;
    /** ER diagram of an object's schema (centred on it), else of the selected schema. */
    showDiagram: (object?: ObjectRef) => void;
    /** Import into a tree object's table, the active table tab's or a new table. */
    importData: (object?: ObjectRef) => void;
    /** Open (or refocus) a scope's process list; the active scope by default. */
    openProcesses: (scope?: TabScope) => void;
    /** Palette commands the active connection cannot run. */
    hiddenCommands: readonly string[];
    toggleSafe: () => void;
    refresh: () => void;
    command: (id: string) => void;
  };
