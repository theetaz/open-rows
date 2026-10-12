// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { SchemaChangeDto } from '@openrows/ipc';
import { newColumn } from './columnEdits';
import type { ColumnRow } from './effective';
import { ColumnOptions, optionChanges } from './forms/ColumnOptions';
import { columnType } from './forms/columnDraft';
import { ColumnFields } from './forms/ColumnFields';
import {
  addColumnChange,
  columnChanges,
  columnProblem,
  draftFromRow,
  emptyColumn,
} from './forms/columnDraft';
import type { ColumnDraft } from './forms/columnDraft';
import type { StageOp } from './forms/staging';
import { StageSheet } from './StageSheet';
import type { Structure } from './useStructure';

/** The staging of a column form: per-property changes, or a new column. */
function columnOp(
  row: ColumnRow | null,
  draft: ColumnDraft,
  original: ColumnDraft,
): StageOp {
  if (row === null) return { kind: 'stage', changes: [addColumnChange(draft)] };
  if (row.column === null)
    return { kind: 'replace', id: row.id, change: addColumnChange(draft) };
  return { kind: 'stage', changes: columnChanges(draft, row.column, original) };
}

function schemaChangesFor(
  structure: Structure,
  row: ColumnRow | null,
): SchemaChangeDto[] {
  return structure.schema.history.current
    .map((item) => item.change)
    .filter((change) => 'column' in change && change.column === row?.column);
}

/**
 * Add or edit one column in a form: every property the engine supports,
 * with the live DDL of all staged changes including this one.
 */
export function ColumnSheet({
  structure,
  row,
  onClose,
}: {
  structure: Structure;
  /** The column to edit; `null` adds one. */
  row: ColumnRow | null;
  onClose: () => void;
}): React.JSX.Element {
  const { engine, features } = structure;
  const [original] = useState(() =>
    row
      ? draftFromRow(row)
      : emptyColumn(newColumn(structure.rows).name, engine),
  );
  const [draft, setDraft] = useState(original);
  const [options, setOptions] = useState(() => {
    const changes = schemaChangesFor(structure, row);
    const conversion = changes.find(
      (change) => change.kind === 'columnTypeUsing',
    );
    const position = changes.find((change) => change.kind === 'reorderColumn');
    return {
      expression: conversion?.expression ?? '',
      position: position
        ? position.after === null
          ? 'first'
          : `after:${position.after}`
        : '',
    };
  });
  const problem = columnProblem(draft);
  const op = columnOp(row, draft, original);
  if (row?.column && op.kind === 'stage') {
    const staged = schemaChangesFor(structure, row);
    const hadUsing = staged.some((change) => change.kind === 'columnTypeUsing');
    if (options.expression.trim() || hadUsing)
      op.changes = op.changes.filter((change) => change.kind !== 'columnType');
    op.changes = [
      ...op.changes,
      ...optionChanges(row.column, columnType(draft), options, hadUsing),
    ];
    if (
      !options.position &&
      staged.some((change) => change.kind === 'reorderColumn')
    )
      op.remove = [`reorderColumn:${row.column}`];
  }
  return (
    <StageSheet
      structure={structure}
      title={row ? `Edit column ${row.name}` : 'Add column'}
      op={problem ? null : op}
      problem={problem}
      stageLabel={row ? 'Stage changes' : 'Stage column'}
      onClose={onClose}
    >
      {row?.column && (
        <ColumnOptions
          structure={structure}
          column={row.column}
          draft={options}
          onChange={setOptions}
        />
      )}
      <ColumnFields
        draft={draft}
        onChange={setDraft}
        engine={engine}
        features={features}
      />
    </StageSheet>
  );
}
