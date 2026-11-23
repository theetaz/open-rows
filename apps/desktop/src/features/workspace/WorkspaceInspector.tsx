// SPDX-License-Identifier: Apache-2.0
import { CellDetails } from '../query';
import type { SelectedCell } from '../query';
import {
  InspectorHead,
  inspectorLabel,
  RowDetails,
  rowTitle,
  TableInfoPanel,
} from '../table';
import type { SelectedRow, SelectedTable } from '../table';
import styles from './Inspector.module.css';

/** `Row 9` and the table for a row; the cell's row and column for a cell. */
function heading(
  row: SelectedRow | null,
  cell: SelectedCell | null,
): { title: string; subtitle: string } | null {
  if (row)
    return {
      title: row.number ? rowTitle(row.number) : '',
      subtitle: row.table,
    };
  if (cell)
    return {
      title: rowTitle(cell.row + 1),
      subtitle: cell.column,
    };
  return null;
}

/**
 * Row or cell details of the active tab, on the prototype's raised inspector
 * card with a `Row 9 · table` heading. History lives in the sidebar.
 */
export function WorkspaceInspector(props: {
  cell: SelectedCell | null;
  row: SelectedRow | null;
  /** A table tab in Structure/Index mode shows its table info instead. */
  table?: SelectedTable | null;
  /** Hides the inspector (the heading's close button). */
  onHide?: (() => void) | undefined;
}): React.JSX.Element {
  const head = props.table ? null : heading(props.row, props.cell);
  return (
    <aside className={styles.inspector} aria-label={inspectorLabel()}>
      {head && (
        <InspectorHead
          {...head}
          fields={props.row?.bulk ? undefined : props.row?.fields}
          onHide={props.onHide}
        />
      )}
      <div className={styles.body}>
        {props.table ? (
          <TableInfoPanel table={props.table} />
        ) : props.row ? (
          <RowDetails row={props.row} />
        ) : (
          <CellDetails cell={props.cell} />
        )}
      </div>
    </aside>
  );
}
