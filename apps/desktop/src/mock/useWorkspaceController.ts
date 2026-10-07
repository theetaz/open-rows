// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { Connections } from '../features/connections';
import { useCatalog } from '../features/schema';
import type { Catalog, ObjectRef } from '../features/schema';
import { useQueryTabs } from '../features/query';
import type { QueryTabs, SelectedCell, TabScope } from '../features/query';
import { queryCommands } from './queryCommands';
import { useCompact, useInspectorOpen } from './layout';
import { useWorkspaceKeys } from './useWorkspaceKeys';
import { useDatabasePrompt } from './useDatabasePrompt';
import type { PaletteMode } from './WorkspacePalettes';
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
/**
 * Tab scopes of the active connection: the active database's scope (the
 * profile's database until one is open, so tabs can be prepared before
 * connecting; none without either) and a mapper for any database.
 */
function scopes(connections: Connections): {
  scope: TabScope | undefined;
  scopeOf: (database: string) => TabScope | undefined;
} {
  const profile = connections.active;
  const scopeOf = (database: string): TabScope | undefined =>
    profile && database ? { connectionId: profile.id, database } : undefined;
  const database = connections.database || (profile?.database ?? '');
  return { scope: scopeOf(database), scopeOf };
}
/** Shell state and command wiring, isolated from rendering. */
export function useWorkspaceController(
  onGallery: () => void,
  enabled: boolean,
  connections: Connections,
): WorkspaceController {
  const catalog = useCatalog(connections);
  const tabs = useQueryTabs();
  const { scope, scopeOf } = scopes(connections);
  const compact = useCompact();
  const inspector = useInspectorOpen(compact);
  const [sidebar, setSidebar] = useState(true);
  const [safe, setSafe] = useState(false);
  const [panel, setPanel] = useState('Details');
  const [mode, setMode] = useState<PaletteMode | null>(null);
  useDatabasePrompt(connections, setMode);
  const [refreshes, setRefreshes] = useState(0);
  const [cell, setCell] = useState<SelectedCell | null>(null);
  const commands = queryCommands(tabs, scope, scopeOf);
  const { addQuery, closeTab } = commands;
  const openObject = (object: ObjectRef): void => {
    catalog.setSchema(object.schema);
    commands.openObject(object);
  };
  const prod = connections.active?.env.kind === 'prod';
  const toggleSafe = (): void => {
    if (!prod) setSafe(!safe);
  };
  const refresh = (): void => {
    catalog.refresh();
    setRefreshes((count) => count + 1);
  };
  const command = shellCommand({
    catalog,
    inspector,
    toggleSidebar: () => {
      setSidebar((open) => !open);
    },
    addQuery,
    closeTab,
    setMode,
    refresh,
    toggleSafe,
    onGallery,
    setSidebar,
    setPanel,
  });
  useWorkspaceKeys({
    enabled: enabled && !mode,
    command,
    connection: connections.selectIndex,
  });
  return {
    catalog,
    tabs,
    scope,
    compact,
    inspector,
    sidebar,
    safe: safe || prod,
    prod,
    panel,
    setPanel,
    mode,
    setMode,
    refreshes,
    cell,
    onCell: (selected) => {
      setCell(selected);
      setPanel('Details');
    },
    toggleSidebar: () => {
      setSidebar((open) => !open);
    },
    addQuery,
    openObject,
    toggleSafe,
    refresh,
    command,
  };
}

type WorkspaceController = {
  catalog: Catalog;
  tabs: QueryTabs;
  /** Tab scope of the browsed connection and database. */
  scope: TabScope | undefined;
  compact: boolean;
  inspector: ReturnType<typeof useInspectorOpen>;
  sidebar: boolean;
  safe: boolean;
  prod: boolean;
  panel: string;
  setPanel: (panel: string) => void;
  mode: PaletteMode | null;
  setMode: (mode: PaletteMode | null) => void;
  refreshes: number;
  cell: SelectedCell | null;
  onCell: (cell: SelectedCell) => void;
  toggleSidebar: () => void;
  /** Open a query tab on the active connection, optionally with SQL. */
  addQuery: (sql?: string) => void;
  /** Open (or refocus) the tab that selects from a tree or ⌘P object. */
  openObject: (object: ObjectRef) => void;
  toggleSafe: () => void;
  refresh: () => void;
  command: (id: string) => void;
};
function shellCommand(input: {
  catalog: Catalog;
  inspector: ReturnType<typeof useInspectorOpen>;
  toggleSidebar: () => void;
  addQuery: () => void;
  closeTab: () => void;
  setMode: (mode: PaletteMode | null) => void;
  refresh: () => void;
  toggleSafe: () => void;
  onGallery: () => void;
  setSidebar: (value: boolean) => void;
  setPanel: (panel: string) => void;
}): (id: string) => void {
  const { catalog, inspector, setMode, setSidebar, setPanel } = input;
  return (id: string): void => {
    const actions: Record<string, (() => void) | undefined> = {
      sidebar: input.toggleSidebar,
      inspector: inspector.toggle,
      'new-tab': input.addQuery,
      'close-tab': input.closeTab,
      database: () => {
        setMode('database');
      },
      quick: () => {
        setMode('quick');
      },
      commands: () => {
        setMode('commands');
      },
      refresh: input.refresh,
      safe: input.toggleSafe,
      gallery: input.onGallery,
      search: () => {
        setSidebar(true);
        requestAnimationFrame(() => {
          document
            .querySelector<HTMLInputElement>('[data-sidebar] input')
            ?.focus();
        });
      },
      schema: () => {
        catalog.setSchema(nextSchema(catalog));
      },
      assistant: () => {
        inspector.show();
        setPanel('Assistant');
      },
    };
    actions[id]?.();
  };
}
