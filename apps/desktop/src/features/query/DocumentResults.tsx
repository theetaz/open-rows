// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { JsonTree } from '../value/JsonTree';
import { useCellValue } from '../value/useCellValue';
import type { ResultMeta } from './run';
import styles from './Query.module.css';

type Mode = 'table' | 'tree' | 'json';

/** Add document controls only to a document result, keeping SQL fields unchanged. */
export function DocumentView({
  result,
  children,
}: {
  result: ResultMeta;
  children: React.ReactNode;
}): React.JSX.Element {
  return result.columns.some(
    (column) => column.name === '__document' && column.sourceType === 'EJSON',
  ) ? (
    <DocumentResults key={result.id} result={result}>
      {children}
    </DocumentResults>
  ) : (
    <>{children}</>
  );
}

/** Document results retain the canvas table and load one lossless document at a time. */
export function DocumentResults({
  result,
  children,
}: {
  result: ResultMeta;
  children: React.ReactNode;
}): React.JSX.Element {
  const [mode, setMode] = useState<Mode>('table');
  const [row, setRow] = useState(0);
  return (
    <div className={styles.resultView}>
      <div role="toolbar" aria-label="Document view">
        {(['table', 'tree', 'json'] as const).map((view) => (
          <Button
            key={view}
            size="sm"
            aria-pressed={mode === view}
            onClick={() => {
              setMode(view);
            }}
          >
            {view}
          </Button>
        ))}
        {mode !== 'table' && (
          <>
            <Button
              size="sm"
              disabled={row === 0}
              onClick={() => {
                setRow(row - 1);
              }}
            >
              Previous document
            </Button>
            <span>
              {String(row + 1)} / {String(result.rows)}
            </span>
            <Button
              size="sm"
              disabled={row + 1 >= result.rows}
              onClick={() => {
                setRow(row + 1);
              }}
            >
              Next document
            </Button>
          </>
        )}
      </div>
      {mode === 'table' ? (
        children
      ) : result.rows > 0 ? (
        <DocumentValue
          key={`${result.id}:${String(row)}`}
          result={result}
          row={row}
          mode={mode}
        />
      ) : (
        <p>No documents.</p>
      )}
    </div>
  );
}
function DocumentValue({
  result,
  row,
  mode,
}: {
  result: ResultMeta;
  row: number;
  mode: Mode;
}): React.JSX.Element {
  const column = result.columns.findIndex(
    (field) => field.name === '__document',
  );
  const value = useCellValue({ resultId: result.id, row, column });
  if (value.problem) return <p role="alert">{value.problem}</p>;
  if (!value.bytes)
    return (
      <Button onClick={value.load} disabled={value.loading}>
        {value.loading ? 'Loading document…' : 'Load document'}
      </Button>
    );
  const text = new TextDecoder().decode(value.bytes);
  let parsed: object;
  try {
    parsed = JSON.parse(text) as object;
  } catch {
    return <pre>{text}</pre>;
  }
  return (
    <div className={styles.grid}>
      {mode === 'tree' ? <JsonTree value={parsed} /> : <pre>{text}</pre>}
    </div>
  );
}
