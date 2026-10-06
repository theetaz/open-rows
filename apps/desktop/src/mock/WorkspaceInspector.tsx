// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { GlassPanel, SegmentedControl } from '@openrows/ui';
import { inspectorSegments } from './data/content';
import type { OrderRow } from './data/rows';
const Assistant = lazy(() =>
  import('./Assistant').then((module) => ({ default: module.Assistant })),
);
const InspectorDetails = lazy(() =>
  import('./InspectorDetails').then((module) => ({
    default: module.InspectorDetails,
  })),
);
import styles from './Inspector.module.css';

/** Row details and assistant share the optional glass inspector surface. */
export function WorkspaceInspector(props: {
  row: OrderRow;
  index: number;
  panel: string;
  onPanel: (panel: string) => void;
  onQuery: (sql: string) => void;
  table: string;
  schema: string;
}): React.JSX.Element {
  return (
    <GlassPanel className={styles.inspector}>
      <SegmentedControl
        aria-label="Inspector mode"
        options={inspectorSegments}
        value={props.panel}
        onValueChange={props.onPanel}
      />
      {props.panel === 'Assistant' ? (
        <Suspense fallback={null}>
          <Assistant onQuery={props.onQuery} />
        </Suspense>
      ) : (
        <Suspense fallback={null}>
          <InspectorDetails {...props} />
        </Suspense>
      )}
    </GlassPanel>
  );
}
