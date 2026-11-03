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
import { t } from '../messages/lazyStructure';

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
      ? t('commitDiscardOther')
      : null);
  return (
    <StageSheet
      structure={structure}
      title={t('addIndex')}
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
      stageLabel={t('stageIndex')}
      note={concurrent ? t('concurrentIndexRuns') : t('indexBuiltWhen')}
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
          {t('buildConcurrently')}
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
