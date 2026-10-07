// SPDX-License-Identifier: Apache-2.0
import { useId, useState } from 'react';
import { Button, Icon } from '@openrows/ui';
import { StagedRows } from './StagedRows';
import type { RowEditing } from './useRowEditing';
import styles from './Table.module.css';
/** Whether rows can be staged: an editable key and an open session. */
export function canEdit(editing: RowEditing, ready: boolean): boolean {
  return Boolean(
    ready &&
    editing.info &&
    !editing.info.readOnly &&
    editing.info.key.length > 0 &&
    !editing.history.uncertain,
  );
}

/** Pending-changes bar, shown only while edits are staged or uncertain. */
export function EditToolbar({
  editing,
  ready,
}: {
  editing: RowEditing;
  ready: boolean;
}): React.JSX.Element | null {
  const { history } = editing;
  const [listOpen, setListOpen] = useState(false);
  const listId = useId();
  if (!history.uncertain && !history.current.length && !history.future.length)
    return null;
  const enabled = canEdit(editing, ready);
  return (
    <>
      {listOpen && <StagedRows editing={editing} id={listId} />}
      <div className={styles.editToolbar} aria-label="Table editing">
        <Button
          size="sm"
          variant="ghost"
          className={styles.editNote}
          aria-expanded={listOpen}
          aria-controls={listOpen ? listId : undefined}
          disabled={!history.current.length}
          onClick={() => {
            setListOpen(!listOpen);
          }}
        >
          <Icon name={listOpen ? 'chevronDown' : 'chevronRight'} />
          {editingNote(editing)}
        </Button>
        <span className={styles.spacer} />
        <Button
          size="sm"
          variant="ghost"
          disabled={!history.past.length || editing.busy}
          onClick={editing.undo}
        >
          Undo
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={!history.future.length || editing.busy}
          onClick={editing.redo}
        >
          Redo
        </Button>
        <Button
          size="sm"
          disabled={!history.current.length || editing.busy}
          onClick={editing.discard}
        >
          {history.uncertain ? 'Reload & discard' : 'Discard'}
        </Button>
        <Button
          size="sm"
          variant="primary"
          disabled={!enabled || !history.current.length || editing.busy}
          onClick={() => {
            editing.commit();
          }}
        >
          Review &amp; commit
        </Button>
      </div>
    </>
  );
}

/** Status of staged edits, or why the table cannot be edited. */
export function editingNote(editing: RowEditing): string {
  if (editing.history.uncertain)
    return 'Commit status unknown; reload before editing.';
  if (editing.history.current.length)
    return `${String(editing.history.current.length)} staged ${editing.history.current.length === 1 ? 'change' : 'changes'}`;
  if (editing.info?.readOnly) return 'Read-only connection';
  if (editing.info && !editing.info.key.length)
    return 'Read-only: no supported non-null unique key';
  return 'Double-click a cell or press Enter to edit · Shift-click a header to select a column';
}
