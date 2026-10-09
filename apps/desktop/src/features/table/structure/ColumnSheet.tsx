// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { newColumn } from './columnEdits';
import type { ColumnRow } from './effective';
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
  const problem = columnProblem(draft);
  return (
    <StageSheet
      structure={structure}
      title={row ? `Edit column ${row.name}` : 'Add column'}
      op={problem ? null : columnOp(row, draft, original)}
      problem={problem}
      stageLabel={row ? 'Stage changes' : 'Stage column'}
      onClose={onClose}
    >
      <ColumnFields
        draft={draft}
        onChange={setDraft}
        engine={engine}
        features={features}
      />
    </StageSheet>
  );
}
