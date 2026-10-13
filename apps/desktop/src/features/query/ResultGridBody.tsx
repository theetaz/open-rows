// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TableGrid } from '../table/TableGrid';
import { RowForm } from '../table/RowForm';
import { ColumnsPanel } from '../table/ColumnsPanel';
import { ResultTools } from './ResultTools';
import { columnKeys } from './resultColumns';
import type { ResultGridModel, ResultGridProps } from './useResultGrid';

/** Presentation toggles surround the same table canvas and row form. */
export function ResultGridBody({
  model,
  result,
  onExport,
}: Pick<ResultGridProps, 'result' | 'onExport'> & {
  model: ResultGridModel;
}): React.JSX.Element {
  const [form, setForm] = useState(false);
  const [row, setRow] = useState(0);
  const [columns, setColumns] = useState(false);
  const { editing } = model.edits;
  return (
    <>
      <ResultTools
        view={model.view}
        ready={model.edits.ready && !editing.busy}
        onInsert={editing.insert}
        onFit={model.fitAll}
        onColumns={() => {
          setColumns(!columns);
        }}
        onForm={() => {
          setRow(model.tools.selection?.cell.row ?? 0);
          setForm(!form);
        }}
        hasRows={model.projected.rows > 0}
        onExport={onExport}
      />
      {columns && (
        <ColumnsPanel
          columns={columnKeys(result.columns)}
          hidden={model.hidden}
          actions={{
            toggleHidden: (name) => {
              model.setHidden(
                model.hidden.includes(name)
                  ? model.hidden.filter((item) => item !== name)
                  : [...model.hidden, name],
              );
            },
          }}
        />
      )}
      {form ? (
        <RowForm
          table={model.edits.target.table || 'Query result'}
          result={model.projected}
          editing={editing}
          ready={model.edits.ready}
          offset={0}
          row={Math.min(row, Math.max(0, model.projected.rows - 1))}
          onRow={setRow}
          onClose={() => {
            setForm(false);
          }}
        />
      ) : (
        <ResultCanvas model={model} />
      )}
    </>
  );
}
function ResultCanvas({
  model,
}: {
  model: ResultGridModel;
}): React.JSX.Element {
  return (
    <TableGrid
      result={model.projected}
      sort={model.sort}
      editing={model.edits.editing}
      offset={0}
      rowNumbers
      striped={model.striped}
      layout={model.layout}
      onLayout={model.changeLayout}
      copyTarget={model.copyTarget}
      links={model.tools.jumps.columns}
      onLink={(cell) => {
        model.tools.jumps.openCell(model.projected, cell);
      }}
      onRow={model.tools.onRow}
      onSelection={model.tools.onSelection}
      onContextMenu={(target) => {
        model.context.show(model.projected, target);
      }}
      onHeader={(column) => {
        model.view.toggleSort(
          model.projected.projection?.columnOf(column) ?? column,
        );
      }}
      onReady={model.onReady}
    />
  );
}
