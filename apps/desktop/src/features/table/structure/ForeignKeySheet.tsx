// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  cleanForeignKey,
  emptyForeignKey,
  ForeignKeyFields,
  foreignKeyProblem,
} from './forms/ForeignKeyFields';
import { StageSheet } from './StageSheet';
import type { Structure } from './useStructure';

/** `Add foreign key…`: columns, referenced table and columns, actions. */
export function ForeignKeySheet({
  structure,
  onClose,
}: {
  structure: Structure;
  onClose: () => void;
}): React.JSX.Element {
  const { target } = structure;
  const [draft, setDraft] = useState(() => emptyForeignKey(target.schema));
  const problem = foreignKeyProblem(draft);
  return (
    <StageSheet
      structure={structure}
      title="Add foreign key"
      op={
        problem
          ? null
          : {
              kind: 'stage',
              changes: [{ kind: 'addForeignKey', key: cleanForeignKey(draft) }],
            }
      }
      problem={problem}
      stageLabel="Stage foreign key"
      onClose={onClose}
    >
      <ForeignKeyFields
        draft={draft}
        onChange={setDraft}
        columns={structure.rows
          .filter((row) => !row.dropped)
          .map((row) => row.name)}
        scope={target}
      />
    </StageSheet>
  );
}
