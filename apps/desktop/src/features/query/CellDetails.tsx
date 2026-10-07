// SPDX-License-Identifier: Apache-2.0
import type { SelectedCell } from './cell';
import styles from './Query.module.css';

/** The focused result cell's value; the full-value inspector arrives with M1-20. */
export function CellDetails({
  cell,
}: {
  cell: SelectedCell | null;
}): React.JSX.Element {
  if (!cell)
    return (
      <p className={styles.empty}>
        Select a cell in a result to see its value.
      </p>
    );
  return (
    <div className={styles.cell}>
      <h2>{cell.column}</h2>
      <small>
        Row {cell.row + 1}
        {cell.type ? ` · ${cell.type}` : ''}
      </small>
      <pre data-null={cell.text === 'NULL'}>{cell.text ?? 'Loading…'}</pre>
    </div>
  );
}
