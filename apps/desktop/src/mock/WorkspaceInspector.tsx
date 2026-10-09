// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { GlassPanel, SegmentedControl } from '@openrows/ui';
import { CellDetails } from '../features/query';
import type { SelectedCell } from '../features/query';
import { RowDetails, TableInfoPanel } from '../features/table';
import type { SelectedRow, SelectedTable } from '../features/table';
const Assistant = lazy(() =>
  import('./Assistant').then((module) => ({ default: module.Assistant })),
);
import styles from './Inspector.module.css';

const segments = ['Details', 'Assistant'].map((label) => ({
  value: label,
  label,
}));

/** Row or cell details and the (sample) assistant share the inspector. History lives in the sidebar. */
export function WorkspaceInspector(props: {
  cell: SelectedCell | null;
  row: SelectedRow | null;
  /** A table tab in Structure/Index mode shows its table info instead. */
  table?: SelectedTable | null;
  panel: string;
  onPanel: (panel: string) => void;
  onQuery: (sql: string) => void;
}): React.JSX.Element {
  return (
    <GlassPanel className={styles.inspector}>
      <SegmentedControl
        aria-label="Inspector mode"
        options={segments}
        value={props.panel}
        onValueChange={props.onPanel}
      />
      {props.panel === 'Assistant' ? (
        <Suspense fallback={null}>
          <Assistant onQuery={props.onQuery} />
        </Suspense>
      ) : props.table ? (
        <TableInfoPanel table={props.table} />
      ) : props.row ? (
        <RowDetails row={props.row} />
      ) : (
        <CellDetails cell={props.cell} />
      )}
    </GlassPanel>
  );
}
