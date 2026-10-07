// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { RowEditing } from './useRowEditing';
import styles from './Table.module.css';
/** Explicit staged-edit actions and eligibility explanation. */
export function EditToolbar({
  editing,
  ready,
}: {
  editing: RowEditing;
  ready: boolean;
}): React.JSX.Element {
  const enabled =
    ready &&
    editing.info &&
    !editing.info.readOnly &&
    editing.info.key.length > 0 &&
    !editing.history.uncertain;
  return (
    <div className={styles.editToolbar} aria-label="Table editing">
      <Button
        size="sm"
        disabled={!enabled || editing.busy}
        onClick={editing.insert}
      >
        + Row
      </Button>
      <span className={styles.editNote}>{editingNote(editing)}</span>
      <span className={styles.spacer} />
      <Button
        size="sm"
        disabled={!editing.history.past.length || editing.busy}
        onClick={editing.undo}
      >
        Undo
      </Button>
      <Button
        size="sm"
        disabled={!editing.history.future.length || editing.busy}
        onClick={editing.redo}
      >
        Redo
      </Button>
      <Button
        size="sm"
        disabled={!editing.history.current.length || editing.busy}
        onClick={editing.discard}
      >
        {editing.history.uncertain ? 'Reload & discard' : 'Discard'}
      </Button>
      <Button
        size="sm"
        variant="primary"
        disabled={!enabled || !editing.history.current.length || editing.busy}
        onClick={() => {
          editing.commit();
        }}
      >
        Review &amp; commit
      </Button>
    </div>
  );
}

function editingNote(editing: RowEditing): string {
  if (editing.history.uncertain)
    return 'Commit status unknown; reload before editing.';
  if (editing.history.current.length)
    return `${String(editing.history.current.length)} staged changes`;
  if (editing.info?.readOnly) return 'Read-only connection';
  if (editing.info && !editing.info.key.length)
    return 'Read-only: no supported non-null unique key';
  return 'Double-click a cell or press Enter to edit · Shift-click a header to select a column';
}
