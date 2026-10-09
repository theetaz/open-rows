// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';
import { togglePin } from '../../results/columnLayout';
import type { ColumnLayout } from '../../results/columnLayout';
import {
  tableLayoutKey,
  useColumnLayout,
} from '../../results/columnLayoutStore';
import type { TableObject } from '../query/tabs';
import { tableTarget } from './model';
import type { GridMenu } from './contextMenuActions';

/** A table's persisted column layout and a function to change it. */
export type TableLayout = {
  layout: ColumnLayout;
  change: (update: (layout: ColumnLayout) => ColumnLayout) => void;
  /** Copy and pin actions of the table's grid menus. */
  grid: GridMenu;
};

/** Column widths, order and pins of one table, kept per table across sessions. */
export function useTableLayout(
  table: TableObject,
  profileId: string,
  engine: EngineDto,
): TableLayout {
  const [layout, change] = useColumnLayout(
    tableLayoutKey(tableTarget(table, profileId)),
  );
  return {
    layout,
    change,
    grid: {
      copyTarget: {
        engine,
        table: { schema: table.schema, name: table.name },
      },
      pinned: layout.pinned,
      togglePin: (name) => {
        change((current) => togglePin(current, name));
      },
    },
  };
}
