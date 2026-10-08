// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { TableObject } from '../../query/tabs';
import type { DetailState } from '../entries';
import type { ViewMode } from '../model';
import { useTrigger } from '../useTableTab';
import type { TableViews } from '../useTableViews';
import type { InlineEdit } from './columnCells';
import { columnRows, indexRows, tableValue } from './effective';
import type { ColumnRow, IndexRow } from './effective';
import type { MenuState } from './ItemMenu';
import { structureActions } from './structureActions';
import type { StructureActions } from './structureActions';
import type { Cursor } from './StructureGrid';
import type { SelectedTable } from './TableInfoPanel';
import { useSchemaEditing } from './useSchemaEditing';
import type { SchemaEditing } from './useSchemaEditing';

type Args = {
  tabId: string;
  table: TableObject;
  profileId: string;
  sessionId: string | null;
  views: TableViews;
  mode: ViewMode;
  onRenamed?: ((name: string) => void) | undefined;
  onOpenTable?: ((schema: string, table: string) => void) | undefined;
  onTableInfo?: ((table: SelectedTable | null) => void) | undefined;
};

/** Everything the Structure and Index modes, their footer and sheets share. */
export type Structure = StructureActions & {
  mode: ViewMode;
  schema: SchemaEditing;
  detail: DetailState;
  rows: ColumnRow[];
  indexes: IndexRow[];
  search: string;
  setSearch: (search: string) => void;
  columnGrid: {
    cursor: Cursor;
    setCursor: (cursor: Cursor) => void;
    edit: InlineEdit | null;
    setEdit: (edit: InlineEdit | null) => void;
  };
  indexGrid: { cursor: Cursor; setCursor: (cursor: Cursor) => void };
  menu: MenuState | null;
  setMenu: (menu: MenuState | null) => void;
  /** The open sheet: triggers or a new index. */
  sheet: 'triggers' | 'index' | null;
  setSheet: (sheet: 'triggers' | 'index' | null) => void;
  openIndexSheet: () => void;
  /** The DDL popover's anchor while it is open. */
  ddl: DOMRect | null;
  toggleDdl: (anchor: DOMRect) => void;
  closeDdl: () => void;
};

const origin: Cursor = { row: 0, col: 1 };

/** State and commands of a table tab's Structure and Index modes. */
export function useStructure(args: Args): Structure {
  const { tabId, table, views } = args;
  const target = {
    profileId: args.profileId,
    database: table.database,
    schema: table.schema,
    table: table.name,
  };
  const schema = useSchemaEditing({ ...args, target });
  const detail = views.entryOf(tabId).detail;
  const staged = schema.history.current;
  const rows = schema.detail ? columnRows(schema.detail, staged) : [];
  const [cursor, setCursor] = useState(origin);
  const [indexCursor, setIndexCursor] = useState(origin);
  const [edit, setEdit] = useState<InlineEdit | null>(null);
  const [search, setSearch] = useState('');
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [sheet, setSheet] = useState<'triggers' | 'index' | null>(null);
  const [ddl, setDdl] = useState<DOMRect | null>(null);
  useTrigger(
    args.mode !== 'data' && detail.status === 'idle' ? 'load' : null,
    () => {
      views.loadDetail(tabId, target);
    },
  );
  usePublishInfo(args, schema, args.mode !== 'data');
  return {
    ...structureActions({
      schema,
      rows,
      setCursor,
      setEdit,
      setSearch,
      onOpenTable: args.onOpenTable,
    }),
    mode: args.mode,
    schema,
    detail,
    rows,
    indexes: schema.detail ? indexRows(schema.detail, staged) : [],
    search,
    setSearch,
    columnGrid: { cursor, setCursor, edit, setEdit },
    indexGrid: { cursor: indexCursor, setCursor: setIndexCursor },
    menu,
    setMenu,
    sheet,
    setSheet,
    openIndexSheet: () => {
      if (schema.enabled) setSheet('index');
    },
    ddl,
    toggleDdl: (anchor) => {
      setDdl((open) => (open ? null : anchor));
    },
    closeDdl: () => {
      setDdl(null);
    },
  };
}

/** Show the table's sizes and comment in the inspector while `show` holds. */
function usePublishInfo(
  args: Args,
  schema: SchemaEditing,
  show: boolean,
): void {
  const { onTableInfo } = args;
  const stage = useRef(schema.stage);
  useEffect(() => {
    stage.current = schema.stage;
  });
  const { detail, enabled } = schema;
  const staged = tableValue(schema.history.current, 'tableComment');
  const comment =
    staged === undefined ? (detail?.info.comment ?? null) : staged.comment;
  const renamed = tableValue(schema.history.current, 'renameTable');
  const name = renamed?.name ?? args.table.name;
  useEffect(() => {
    if (!onTableInfo || !show || !detail) return;
    onTableInfo({
      name,
      info: detail.info,
      comment,
      onComment: enabled
        ? (next) => {
            stage.current({ kind: 'tableComment', comment: next });
          }
        : undefined,
    });
    return () => {
      onTableInfo(null);
    };
  }, [onTableInfo, show, detail, comment, enabled, name]);
}
