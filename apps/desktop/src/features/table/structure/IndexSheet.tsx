// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  cleanIndex,
  emptyIndex,
  IndexFields,
  indexProblem,
} from './forms/IndexFields';
import { StageSheet } from './StageSheet';
import type { Structure } from './useStructure';

/**
 * `+ Index`: name, key columns with their order, unique, method and a
 * partial predicate, with the live DDL of the staged changes and this index.
 */
export function IndexSheet({
  structure,
  onClose,
}: {
  structure: Structure;
  onClose: () => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState(emptyIndex);
  const problem = indexProblem(draft);
  return (
    <StageSheet
      structure={structure}
      title="Add index"
      op={
        problem
          ? null
          : {
              kind: 'stage',
              changes: [{ kind: 'createIndex', ...cleanIndex(draft) }],
            }
      }
      problem={problem}
      stageLabel="Stage index"
      note="The index is built when you commit; building blocks writes to the table until it finishes."
      onClose={onClose}
    >
      <IndexFields
        draft={draft}
        onChange={setDraft}
        columns={structure.rows
          .filter((row) => !row.dropped)
          .map((row) => row.name)}
        engine={structure.engine}
        features={structure.features}
      />
    </StageSheet>
  );
}
