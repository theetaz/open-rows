// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import type { EngineDto, StatementDto } from '@openrows/ipc';
import type { ResultMeta } from './run';
import type { SelectedCell } from './cell';
import styles from './Query.module.css';
const ResultGrid = lazy(() =>
  import('./ResultGrid').then((module) => ({ default: module.ResultGrid })),
);

type GridProps = {
  engine: EngineDto;
  statements?: readonly StatementDto[] | undefined;
  onCell: (cell: SelectedCell) => void;
  /** Export… of some rows of `result`; omitted while unavailable. */
  onExport?: ((result: ResultMeta, rows?: number[]) => void) | undefined;
};

/** `Result 2 · statement 3`: the caption of a result tab or stacked section. */
export function resultLabel(result: ResultMeta, index: number): string {
  return `Result ${String(index + 1)} · statement ${String(result.statement + 1)}`;
}

/** One result's grid once rows arrive, else how many have arrived so far. */
export function ResultContent({
  result,
  engine,
  onCell,
  onExport,
  statements,
}: GridProps & { result: ResultMeta }): React.JSX.Element {
  if (!result.done && result.rows === 0)
    return (
      <p className={styles.placeholder} role="status">
        Receiving rows… {result.rows.toLocaleString()} so far
      </p>
    );
  return (
    <Suspense fallback={null}>
      <ResultGrid
        key={result.id}
        result={result}
        engine={engine}
        statement={statements?.[result.statement]?.sql}
        statementCount={statements?.length ?? 1}
        onExport={
          result.done && onExport
            ? () => {
                onExport(result);
              }
            : undefined
        }
        onCell={onCell}
        onExportRows={
          result.done && onExport
            ? (rows) => {
                onExport(result, rows);
              }
            : undefined
        }
      />
    </Suspense>
  );
}

/** Every result of a multi-statement run, stacked vertically (FR-EDIT-15). */
export function StackedResults(
  props: GridProps & { results: readonly ResultMeta[] },
): React.JSX.Element {
  return (
    <div className={styles.stack}>
      {props.results.map((result, index) => (
        <section
          key={result.id}
          className={styles.stackItem}
          aria-label={resultLabel(result, index)}
        >
          <h4 className={styles.stackTitle}>{resultLabel(result, index)}</h4>
          <ResultContent {...props} result={result} />
        </section>
      ))}
    </div>
  );
}
