// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { GlassPanel, SegmentedControl } from '@openrows/ui';
import { CellDetails, HistoryPanel } from '../features/query';
import type { SelectedCell } from '../features/query';
const Assistant = lazy(() =>
  import('./Assistant').then((module) => ({ default: module.Assistant })),
);
import styles from './Inspector.module.css';

const segments = ['Details', 'History', 'Assistant'].map((label) => ({
  value: label,
  label,
}));

/** Cell details, query history and the (sample) assistant share the inspector. */
export function WorkspaceInspector(props: {
  cell: SelectedCell | null;
  connectionId: string | undefined;
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
      ) : props.panel === 'History' ? (
        <HistoryPanel
          connectionId={props.connectionId}
          onOpen={props.onQuery}
        />
      ) : (
        <CellDetails cell={props.cell} />
      )}
    </GlassPanel>
  );
}
