// SPDX-License-Identifier: Apache-2.0
import { GlassPanel, SegmentedControl } from '@openrows/ui';
import { inspectorSegments } from './data/content';
import { columns, isEdited } from './data/rows';
import type { OrderRow } from './data/rows';
import { Assistant } from './Assistant';
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
        <Assistant onQuery={props.onQuery} />
      ) : (
        <div className={styles.details}>
          <div>
            <h2>Order {props.row[0]}</h2>
            <small>
              Row {props.index + 1} · {props.schema}.{props.table}
            </small>
          </div>
          <dl className={styles.fields}>
            {columns.map((column, index) => (
              <div key={column}>
                <dt>{column}</dt>
                <dd
                  data-null={props.row[index] === null}
                  title={String(props.row[index] ?? 'NULL')}
                >
                  {isEdited(props.index, index) && (
                    <span className={styles.edited} aria-label="Edited field">
                      ✎{' '}
                    </span>
                  )}
                  {props.row[index] ?? 'NULL'}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      )}
    </GlassPanel>
  );
}
