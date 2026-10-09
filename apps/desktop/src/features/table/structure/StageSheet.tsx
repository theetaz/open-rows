// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { DdlPreview } from './forms/DdlPreview';
import { applyOp, plannedChanges } from './forms/staging';
import type { StageOp } from './forms/staging';
import { useAlterPreview } from './forms/usePreview';
import type { Structure } from './useStructure';
import styles from './forms/Forms.module.css';

/**
 * A structure form sheet: its fields, the live DDL of every staged change
 * plus this one, and Stage. `op` is `null` while the form is incomplete
 * (`problem` says why).
 */
export function StageSheet(props: {
  structure: Structure;
  title: string;
  op: StageOp | null;
  problem: string | null;
  stageLabel: string;
  note?: string;
  onClose: () => void;
  children: ReactNode;
}): React.JSX.Element {
  const { structure, op, problem } = props;
  const { schema } = structure;
  const preview = useAlterPreview(
    structure.target,
    plannedChanges(schema, op, structure.target.table),
  );
  return (
    <Sheet title={props.title} onClose={props.onClose}>
      {props.children}
      {problem && <p className={styles.problem}>{problem}</p>}
      <DdlPreview state={preview} />
      <p className={styles.note}>
        {props.note ??
          'The change is staged; commit it from the pending-changes bar.'}
      </p>
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!op || !schema.enabled}
          onClick={() => {
            if (op) applyOp(schema, op);
            props.onClose();
          }}
        >
          {props.stageLabel}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
