// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { GridContextTarget, SelectionModel } from '@openrows/grid';
import { togglePin, withWidth } from '../../results/columnLayout';
import type { CopyFormat } from '../../results/copy/formats';
import { QuickFilter } from '../table/filters/QuickFilter';
import { COLUMN_OPERATORS } from './columnFilters';
import { ResultContextMenu } from './ResultContextMenu';
import { columnKeys } from './resultColumns';
import type { ResultLatest } from './useResultCanvas';
import type { ResultView } from './useResultView';
import type { ResultMeta } from './run';

/** A right-click: the grid target, its selection and the result column. */
export type Menu = GridContextTarget & {
  ranges: SelectionModel['ranges'];
  stored: number;
};

/** What the grid's menus act with, before a right-click picks the cells. */
type GridActions = Omit<MenuActions, 'copy' | 'exportRows' | 'quickFilter'> & {
  copy: (format: CopyFormat, ranges: SelectionModel['ranges']) => void;
  exportRows: ((ranges: SelectionModel['ranges']) => void) | undefined;
};

/** The open context menu and the header quick filter it can open. */
export function ResultMenus(props: {
  menu: Menu | null;
  result: ResultMeta;
  view: ResultView;
  pinned: readonly string[];
  actions: GridActions;
}): React.JSX.Element {
  const { menu, actions } = props;
  const [quick, setQuick] = useState<Menu | null>(null);
  const { exportRows } = actions;
  return (
    <>
      {menu && (
        <MenuFor
          {...props}
          menu={menu}
          actions={{
            ...actions,
            copy: (format) => {
              actions.copy(format, menu.ranges);
            },
            exportRows:
              exportRows &&
              (() => {
                exportRows(menu.ranges);
              }),
            quickFilter: () => {
              setQuick(menu);
            },
          }}
        />
      )}
      {quick && (
        <ResultQuickFilter
          menu={quick}
          result={props.result}
          view={props.view}
          onClose={() => {
            setQuick(null);
          }}
        />
      )}
    </>
  );
}

/** The header quick filter of one result column, applied to loaded rows. */
function ResultQuickFilter({
  menu,
  result,
  view,
  onClose,
}: {
  menu: Menu;
  result: ResultMeta;
  view: ResultView;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <QuickFilter
      name={result.columns[menu.stored]?.name ?? ''}
      x={menu.x}
      y={menu.y}
      operators={COLUMN_OPERATORS}
      onApply={(operator, value) => {
        view.filterColumn({ column: menu.stored, operator, value });
      }}
      onClose={onClose}
    />
  );
}

type MenuActions = {
  copy: (format: CopyFormat) => void;
  copyText: (text: string) => void;
  exportRows: (() => void) | undefined;
  changeLayout: ResultLatest['changeLayout'];
  /** Open the header quick filter of the right-clicked column. */
  quickFilter: () => void;
  fitWidth: (column: number) => number | undefined;
  reset: () => void;
  close: () => void;
};

/** The context menu for one right-click, in result coordinates. */
function MenuFor(props: {
  menu: Menu;
  result: ResultMeta;
  view: ResultView;
  pinned: readonly string[];
  actions: MenuActions;
}): React.JSX.Element {
  const { menu, result, view, actions } = props;
  const column = menu.stored;
  const key = columnKeys(result.columns)[column] ?? '';
  return (
    <ResultContextMenu
      x={menu.x}
      y={menu.y}
      kind={menu.kind}
      name={result.columns[column]?.name ?? ''}
      column={column}
      sorted={view.sorted}
      pinned={props.pinned.includes(key)}
      copy={actions.copy}
      copyText={actions.copyText}
      exportRows={actions.exportRows}
      sortBy={view.sortBy}
      filterColumn={actions.quickFilter}
      togglePin={() => {
        actions.changeLayout((layout) => togglePin(layout, key));
      }}
      fit={() => {
        const width = actions.fitWidth(menu.column);
        if (width !== undefined)
          actions.changeLayout((layout) => withWidth(layout, key, width));
      }}
      resetColumns={actions.reset}
      onClose={actions.close}
    />
  );
}
