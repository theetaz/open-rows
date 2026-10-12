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
  const [concurrent, setConcurrent] = useState(false);
  const problem =
    indexProblem(draft) ??
    (concurrent && structure.schema.history.current.length
      ? 'Commit or discard other changes before staging a concurrent index.'
      : null);
  return (
    <StageSheet
      structure={structure}
      title="Add index"
      op={
        problem
          ? null
          : {
              kind: 'stage',
              changes: [
                concurrent
                  ? {
                      kind: 'createIndexConcurrently',
                      index: cleanIndex(draft),
                    }
                  : { kind: 'createIndex', ...cleanIndex(draft) },
              ],
            }
      }
      problem={problem}
      stageLabel="Stage index"
      note={
        concurrent
          ? 'The concurrent index runs alone outside a transaction; failure may leave an invalid index.'
          : 'The index is built when you commit; building blocks writes to the table until it finishes.'
      }
      onClose={onClose}
    >
      {structure.features.concurrentIndexes && (
        <label>
          <input
            type="checkbox"
            checked={concurrent}
            onChange={(event) => {
              setConcurrent(event.target.checked);
            }}
          />
          Build concurrently
        </label>
      )}
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
