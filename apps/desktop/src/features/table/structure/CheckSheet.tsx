// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { CheckFields, checkProblem, cleanCheck } from './forms/CheckFields';
import { StageSheet } from './StageSheet';
import type { Structure } from './useStructure';

/** `Add check…`: a named or automatic check constraint. */
export function CheckSheet({
  structure,
  onClose,
}: {
  structure: Structure;
  onClose: () => void;
}): React.JSX.Element {
  const [check, setCheck] = useState({
    name: null as string | null,
    expression: '',
  });
  const problem = checkProblem(check);
  return (
    <StageSheet
      structure={structure}
      title="Add check"
      op={
        problem
          ? null
          : {
              kind: 'stage',
              changes: [{ kind: 'addCheck', check: cleanCheck(check) }],
            }
      }
      problem={problem}
      stageLabel="Stage check"
      onClose={onClose}
    >
      <CheckFields check={check} onChange={setCheck} />
    </StageSheet>
  );
}
