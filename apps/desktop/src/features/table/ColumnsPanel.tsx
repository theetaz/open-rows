// SPDX-License-Identifier: Apache-2.0
import type { TableActions } from './useTableTab';
import styles from './Table.module.css';

/** Show or hide columns; hidden columns are left out of the page's SELECT. */
export function ColumnsPanel({
  columns,
  hidden,
  actions,
}: {
  columns: readonly string[];
  hidden: readonly string[];
  actions: TableActions;
}): React.JSX.Element {
  const visible = columns.length - hidden.length;
  return (
    <fieldset className={styles.columns} aria-label="Columns">
      <legend>Columns</legend>
      {columns.map((column) => {
        const shown = !hidden.includes(column);
        return (
          <label key={column}>
            <input
              type="checkbox"
              className={styles.check}
              checked={shown}
              disabled={shown && visible === 1}
              onChange={() => {
                actions.toggleHidden(column);
              }}
            />
            {column}
          </label>
        );
      })}
    </fieldset>
  );
}
