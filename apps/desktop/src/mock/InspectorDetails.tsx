// SPDX-License-Identifier: Apache-2.0
import { columns, isEdited } from './data/rows';
import type { OrderRow } from './data/rows';
import styles from './Inspector.module.css';
/** Row fields mount only when Details is selected. */
export function InspectorDetails(props: {
  row: OrderRow;
  index: number;
  schema: string;
  table: string;
}): React.JSX.Element {
  return (
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
  );
}
