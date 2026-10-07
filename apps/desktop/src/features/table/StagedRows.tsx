// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { RowEditing } from './useRowEditing';
import styles from './Table.module.css';
/** Inserted/deleted rows remain individually reviewable before commit. */
export function StagedRows({
  editing,
  id,
}: {
  editing: RowEditing;
  id: string;
}): React.JSX.Element | null {
  if (!editing.history.current.length) return null;
  return (
    <div id={id} className={styles.stagedRows} aria-label="Staged rows">
      {editing.history.current.map((edit) => (
        <div key={edit.id}>
          <span>
            {edit.kind === 'insert' ? 'New row' : `Row ${String(edit.number)}`}{' '}
            · {edit.kind}
          </span>
          <Button
            size="sm"
            disabled={editing.busy}
            onClick={() => {
              editing.editStaged(edit);
            }}
          >
            Review row
          </Button>
        </div>
      ))}
    </div>
  );
}
