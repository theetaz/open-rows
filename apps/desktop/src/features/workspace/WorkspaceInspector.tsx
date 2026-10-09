// SPDX-License-Identifier: Apache-2.0
import { GlassPanel } from '@openrows/ui';
import { CellDetails } from '../query';
import type { SelectedCell } from '../query';
import { RowDetails, TableInfoPanel } from '../table';
import type { SelectedRow, SelectedTable } from '../table';
import styles from './Inspector.module.css';

/** Row or cell details of the active tab. History lives in the sidebar. */
export function WorkspaceInspector(props: {
  cell: SelectedCell | null;
  row: SelectedRow | null;
  /** A table tab in Structure/Index mode shows its table info instead. */
  table?: SelectedTable | null;
}): React.JSX.Element {
  return (
    <GlassPanel className={styles.inspector}>
      {props.table ? (
        <TableInfoPanel table={props.table} />
      ) : props.row ? (
        <RowDetails row={props.row} />
      ) : (
        <CellDetails cell={props.cell} />
      )}
    </GlassPanel>
  );
}
