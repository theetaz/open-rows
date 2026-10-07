// SPDX-License-Identifier: Apache-2.0
import type { TableStructureDto } from '@openrows/ipc';
import type { StructureState } from './entries';
import styles from './Table.module.css';

const yes = (flag: boolean): string => (flag ? 'YES' : 'NO');

function Columns({ data }: { data: TableStructureDto }): React.JSX.Element {
  return (
    <table className={styles.structure} aria-label="Columns">
      <thead>
        <tr>
          <th>column_name</th>
          <th>data_type</th>
          <th>is_nullable</th>
          <th>column_default</th>
          <th>comment</th>
        </tr>
      </thead>
      <tbody>
        {data.columns.map((column) => (
          <tr key={column.name}>
            <td>{column.name}</td>
            <td>{column.dataType}</td>
            <td>{yes(column.nullable)}</td>
            <td data-null={column.default === null}>
              {column.default ?? 'NULL'}
            </td>
            <td data-null={column.comment === null}>
              {column.comment ?? 'NULL'}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Indexes({ data }: { data: TableStructureDto }): React.JSX.Element {
  if (!data.indexes.length) return <p className={styles.note}>No indexes</p>;
  return (
    <table className={styles.structure} aria-label="Indexes">
      <thead>
        <tr>
          <th>index_name</th>
          <th>columns</th>
          <th>is_unique</th>
          <th>is_primary</th>
          <th>method</th>
          <th>condition</th>
        </tr>
      </thead>
      <tbody>
        {data.indexes.map((index) => (
          <tr key={index.name}>
            <td>{index.name}</td>
            <td>{index.columns}</td>
            <td>{yes(index.unique)}</td>
            <td>{yes(index.primary)}</td>
            <td>{index.method}</td>
            <td data-null={index.predicate === null}>
              {index.predicate ?? 'NULL'}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Structure (columns) or Indexes tab of a table view (T01). */
export function StructureView({
  structure,
  show,
}: {
  structure: StructureState;
  show: 'structure' | 'indexes';
}): React.JSX.Element {
  if (structure.status === 'error')
    return (
      <p role="alert" className={styles.error}>
        {structure.message}
      </p>
    );
  if (structure.status !== 'ready')
    return (
      <p role="status" className={styles.note}>
        Loading structure…
      </p>
    );
  return (
    <div className={styles.scroll}>
      {show === 'structure' ? (
        <Columns data={structure.data} />
      ) : (
        <Indexes data={structure.data} />
      )}
    </div>
  );
}
