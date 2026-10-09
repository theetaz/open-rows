// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { SegmentedControl } from '@openrows/ui';
import { RowDetails } from '../table';
import { readRow } from '../table/row';
import type { RowField, SelectedRow } from '../table/row';
import { ValueInspector } from '../value/ValueInspector';
import type { SelectedCell } from './cell';
import type { ResultMeta } from './run';
import styles from './Query.module.css';

type View = 'value' | 'row';

const views: readonly { value: View; label: string }[] = [
  { value: 'value', label: 'Value' },
  { value: 'row', label: 'Row' },
];

/** The focused result cell: its full value, or every column of its row. */
export function CellDetails({
  cell,
}: {
  cell: SelectedCell | null;
}): React.JSX.Element {
  const [view, setView] = useState<View>('value');
  if (!cell)
    return (
      <p className={styles.empty}>
        Select a cell in a result to see its value.
      </p>
    );
  return (
    <div className={styles.cell}>
      <SegmentedControl
        aria-label="Details view"
        options={views}
        value={view}
        onValueChange={(value) => {
          setView(value === 'row' ? 'row' : 'value');
        }}
      />
      {view === 'value' ? (
        <>
          <h2>{cell.column}</h2>
          <small>Row {cell.row + 1}</small>
          <ValueInspector
            key={`${cell.result.id}:${String(cell.row)}:${String(cell.index)}`}
            address={{
              resultId: cell.result.id,
              row: cell.row,
              column: cell.index,
            }}
            name={cell.column}
            type={cell.type}
            text={cell.text}
          />
        </>
      ) : (
        <ResultRow
          key={`${cell.result.id}:${String(cell.row)}`}
          result={cell.result}
          row={cell.row}
        />
      )}
    </div>
  );
}

/** Every column of one result row, read-only, with full-value inspection per field. */
function ResultRow(props: {
  result: ResultMeta;
  row: number;
}): React.JSX.Element {
  const { result, row } = props;
  const [fields, setFields] = useState<readonly RowField[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    readRow(result, row).then(
      (found) => {
        if (live) setFields(found);
      },
      (error: unknown) => {
        if (live)
          setProblem(error instanceof Error ? error.message : String(error));
      },
    );
    return () => {
      live = false;
    };
  }, [result, row]);
  if (problem) return <p role="alert">{problem}</p>;
  if (!fields) return <p role="status">Loading row…</p>;
  const selected: SelectedRow = {
    table: 'Result',
    number: row + 1,
    fields,
    source: { resultId: result.id, row },
  };
  return <RowDetails row={selected} />;
}
