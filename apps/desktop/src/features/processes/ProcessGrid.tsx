// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Badge } from '@openrows/ui';
import type { ProcessDto, ProcessSignalDto } from '@openrows/ipc';
import { StructureGrid } from '../table/structure/StructureGrid';
import type { CellView, Cursor } from '../table/structure/StructureGrid';
import { ItemMenu } from '../table/structure/ItemMenu';
import type { MenuItem, MenuState } from '../table/structure/ItemMenu';
import { PROCESS_COLUMNS, cellText, processLabel } from './processModel';
import type { ProcessSort } from './processModel';
import styles from './Processes.module.css';

const GRID_COLUMNS = PROCESS_COLUMNS.map(({ key, title, width }) => ({
  key,
  title,
  width,
}));

type Props = {
  processes: readonly ProcessDto[];
  cursor: Cursor;
  onCursor: (cursor: Cursor) => void;
  sort: ProcessSort;
  onSort: (col: number) => void;
  /** Signals are refused on read-only connections, so their items are disabled. */
  readOnly: boolean;
  onSignal: (process: ProcessDto, signal: ProcessSignalDto) => void;
};

/** The list's own connection and the app's other connections carry a badge. */
function applicationCell(process: ProcessDto): CellView {
  const text = cellText(process, 'application');
  const badge = process.current ? 'This list' : process.app ? 'OpenRows' : '';
  return {
    content: (
      <span className={styles.cell}>
        <span>{text}</span>
        {badge && <Badge tone="info">{badge}</Badge>}
      </span>
    ),
    muted: !text && !badge,
  };
}

function cell(process: ProcessDto | undefined, col: number): CellView {
  const key = PROCESS_COLUMNS[col]?.key;
  if (!process || !key) return { content: '' };
  if (key === 'application') return applicationCell(process);
  const text = cellText(process, key);
  return { content: text || 'NULL', muted: !text };
}

/** Copy Query, Cancel Query and Kill Process for one process. */
function processMenuItems(
  process: ProcessDto,
  props: Pick<Props, 'readOnly' | 'onSignal'>,
): MenuItem[] {
  const refused = props.readOnly ? 'This connection is read-only' : undefined;
  return [
    {
      label: 'Copy Query',
      disabled: !process.query,
      run: () => {
        navigator.clipboard
          .writeText(process.query ?? '')
          .catch(() => undefined);
      },
    },
    {
      label: 'Cancel Query',
      disabled: props.readOnly,
      ...(refused ? { title: refused } : {}),
      run: () => {
        props.onSignal(process, 'cancelQuery');
      },
    },
    {
      label: 'Kill Process',
      danger: true,
      disabled: props.readOnly,
      ...(refused ? { title: refused } : {}),
      run: () => {
        props.onSignal(process, 'terminate');
      },
    },
  ];
}

/** Sortable, keyboard-navigable process list with a context menu per row. */
export function ProcessGrid(props: Props): React.JSX.Element {
  const [menu, setMenu] = useState<MenuState | null>(null);
  const { processes } = props;
  const openMenu = (row: number, at: { x: number; y: number }): void => {
    const process = processes[row];
    if (process)
      setMenu({
        ...at,
        label: `Actions for ${processLabel(process)}`,
        items: processMenuItems(process, props),
      });
  };
  return (
    <>
      <StructureGrid
        label="Processes"
        columns={GRID_COLUMNS}
        rows={processes.length}
        rowKey={(row) => String(processes[row]?.id ?? row)}
        rowState={() => undefined}
        cell={(row, col) => cell(processes[row], col)}
        cursor={props.cursor}
        onCursor={props.onCursor}
        onKey={() => false}
        onEdit={() => undefined}
        onMenu={openMenu}
        sort={props.sort}
        onSort={props.onSort}
      />
      {menu && (
        <ItemMenu
          menu={menu}
          onClose={() => {
            setMenu(null);
          }}
        />
      )}
    </>
  );
}
