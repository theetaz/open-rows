// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { PendingAttach } from './useAttachDatabase';
import styles from './AttachDatabase.module.css';

/**
 * Names the schema a chosen `SQLite` file is attached under; Return
 * attaches, Escape cancels. The file itself was chosen in the open panel.
 */
export function AttachDatabaseSheet(props: {
  pending: PendingAttach;
  onAlias: (alias: string) => void;
  onAttach: () => void;
  onCancel: () => void;
}): React.JSX.Element {
  const { pending } = props;
  return (
    <Sheet title="Attach database" size="alert" onClose={props.onCancel}>
      <p className={styles.path} title={pending.path}>
        {pending.path}
      </p>
      <TextField
        label="Schema name"
        value={pending.alias}
        spellCheck={false}
        {...(pending.error ? { error: pending.error } : {})}
        onChange={(event) => {
          props.onAlias(event.target.value);
        }}
      />
      <SheetActions>
        <Button onClick={props.onCancel}>Cancel</Button>
        <Button
          variant="primary"
          disabled={pending.busy || !pending.alias.trim()}
          onClick={props.onAttach}
        >
          Attach
        </Button>
      </SheetActions>
    </Sheet>
  );
}
