// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { ObjectChangeDto } from '@openrows/ipc';
import { DdlPreview } from '../table/structure/forms/DdlPreview';
import { useObjectChange } from './useObjectChange';
import type { ChangeScope, ObjectChangeState } from './useObjectChange';
import styles from '../table/structure/forms/Forms.module.css';

type Props = {
  title: string;
  scope: ChangeScope;
  /** The change the form describes; `null` while it is incomplete. */
  change: ObjectChangeDto | null;
  /** Why the form is incomplete. */
  problem?: string | null;
  /** The default button, e.g. `Create table` or `Drop`. */
  action: string;
  /** Drops and empties use the destructive button. */
  destructive?: boolean;
  size?: 'form' | 'editor';
  onClose: () => void;
  /** After the change ran. */
  onDone: (change: ObjectChangeDto) => void;
  children?: ReactNode;
};

/** How the reviewed statements run, and the name to type when asked. */
function Confirmation({
  state,
}: {
  state: ObjectChangeState;
}): React.JSX.Element | null {
  const { preview } = state;
  if (preview.status !== 'ready') return null;
  return (
    <>
      <p className={styles.note}>
        {preview.transactional
          ? 'The statements run in one transaction: an error rolls every one back.'
          : 'The statements run one at a time, outside a transaction.'}
      </p>
      {preview.confirmName !== null && (
        <TextField
          label={`This connection confirms destructive changes. Type ${preview.confirmName} to continue.`}
          value={state.typed}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => {
            state.setTyped(event.target.value);
          }}
        />
      )}
    </>
  );
}

/**
 * An object editor sheet: its form, the live SQL that will run (with
 * warnings and any confirmation), and the action that runs exactly that.
 */
export function ObjectSheet(props: Props): React.JSX.Element {
  const state = useObjectChange(props.scope, props.change, props.onDone);
  return (
    <Sheet
      title={props.title}
      size={props.size ?? 'form'}
      onClose={props.onClose}
    >
      {props.children}
      {props.problem && <p className={styles.problem}>{props.problem}</p>}
      <DdlPreview state={state.preview}>
        <Confirmation state={state} />
      </DdlPreview>
      {state.error && (
        <p className={styles.problem} role="alert">
          {state.error}
        </p>
      )}
      <SheetActions>
        <Button disabled={state.busy} onClick={props.onClose}>
          Cancel
        </Button>
        <Button
          variant={props.destructive ? 'destructive' : 'primary'}
          disabled={!state.ready}
          onClick={state.run}
        >
          {state.busy ? 'Running…' : props.action}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
