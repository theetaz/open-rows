// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import type { Connections } from '../features/connections';
import { useCatalog } from '../features/schema';
import type { Catalog, ObjectRef } from '../features/schema';
import { rows } from './data/rows';
import type { OrderRow } from './data/rows';
import { useWorkspace } from './useWorkspace';
import { useCompact, useInspectorOpen } from './layout';
import { useWorkspaceKeys } from './useWorkspaceKeys';
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
/** Open an object path in a tab and show its schema in the tree. */
function opener(
  catalog: Catalog,
  state: ReturnType<typeof useWorkspace>,
): (object: ObjectRef) => void {
  return (object) => {
    catalog.setSchema(object.schema);
    state.openTable(object);
  };
}
/** Preview shell state and command wiring, isolated from rendering. */
export function useWorkspaceController(
  onGallery: () => void,
  enabled: boolean,
  connections: Connections,
): WorkspaceController {
  const catalog = useCatalog(connections);
  const state = useWorkspace(
    `${connections.active?.id ?? ''}\n${catalog.database}`,
  );
  const compact = useCompact();
  const inspector = useInspectorOpen(compact);
  const [sidebar, setSidebar] = useState(true);
  const [safe, setSafe] = useState(false);
  const [panel, setPanel] = useState('Assistant');
  const [mode, setMode] = useState<PaletteMode | null>(null);
  const [refreshes, setRefreshes] = useState(0);
  const [selection, setSelection] = useState({ index: 3, row: rows[3] });
  const onSelect = useCallback((index: number, row: OrderRow) => {
    setSelection({ index, row });
    setPanel('Details');
  }, []);
  const toggleSidebar = (): void => {
    setSidebar((open) => !open);
  };
  const { addTab } = state;
  const addQuery = useCallback(() => {
    addTab();
  }, [addTab]);
  const prod = connections.active?.env.kind === 'prod';
  const toggleSafe = (): void => {
    if (!prod) setSafe(!safe);
  };
  const refresh = (): void => {
    catalog.refresh();
    setRefreshes((count) => count + 1);
  };
  const openObject = opener(catalog, state);
  const command = mockCommand({
    state,
    catalog,
    inspector,
    toggleSidebar,
    addQuery,
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
    state,
    catalog,
    openObject,
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
    selection,
    onSelect,
    toggleSidebar,
    addQuery,
    toggleSafe,
    refresh,
    command,
  };
}
type WorkspaceController = {
  state: ReturnType<typeof useWorkspace>;
  catalog: Catalog;
  openObject: (object: ObjectRef) => void;
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
  selection: { index: number; row: OrderRow | undefined };
  onSelect: (index: number, row: OrderRow) => void;
  toggleSidebar: () => void;
  addQuery: () => void;
  toggleSafe: () => void;
  refresh: () => void;
  command: (id: string) => void;
};
function mockCommand(input: {
  state: ReturnType<typeof useWorkspace>;
  catalog: Catalog;
  inspector: ReturnType<typeof useInspectorOpen>;
  toggleSidebar: () => void;
  addQuery: () => void;
  setMode: (mode: PaletteMode | null) => void;
  refresh: () => void;
  toggleSafe: () => void;
  onGallery: () => void;
  setSidebar: (value: boolean) => void;
  setPanel: (panel: string) => void;
}): (id: string) => void {
  const {
    state,
    catalog,
    inspector,
    toggleSidebar,
    addQuery,
    setMode,
    refresh,
    toggleSafe,
    onGallery,
    setSidebar,
    setPanel,
  } = input;
  return (id: string): void => {
    const actions: Record<string, (() => void) | undefined> = {
      sidebar: toggleSidebar,
      inspector: inspector.toggle,
      'new-tab': addQuery,
      'close-tab': () => {
        state.closeTab(state.active);
      },
      database: () => {
        setMode('database');
      },
      quick: () => {
        setMode('quick');
      },
      commands: () => {
        setMode('commands');
      },
      refresh,
      safe: toggleSafe,
      gallery: onGallery,
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
