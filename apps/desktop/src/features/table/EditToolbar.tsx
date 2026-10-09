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

/** What the pending-changes bar shows and does (row or structure edits). */
export type EditBarModel = {
  count: number;
  note: string;
  canUndo: boolean;
  canRedo: boolean;
  uncertain: boolean;
  busy: boolean;
  /** Commit is possible (editable, session open, outcome known). */
  enabled: boolean;
  undo: () => void;
  redo: () => void;
  discard: () => void;
  commit: () => void;
  /** The expandable list of staged items. */
  list: (id: string) => React.ReactNode;
};

/** The bar model of staged row edits; `discard` asks before dropping them. */
export function rowEditBar(
  editing: RowEditing,
  ready: boolean,
  discard: () => void = editing.discard,
): EditBarModel {
  const { history } = editing;
  return {
    count: history.current.length,
    note: editingNote(editing),
    canUndo: history.past.length > 0,
    canRedo: history.future.length > 0,
    uncertain: Boolean(history.uncertain),
    busy: editing.busy,
    enabled: canEdit(editing, ready),
    undo: editing.undo,
    redo: editing.redo,
    discard,
    commit: () => {
      editing.commit();
    },
    list: (id) => <StagedRows editing={editing} id={id} />,
  };
}

/** Pending-changes bar, shown only while edits are staged or uncertain. */
export function EditToolbar({
  model,
}: {
  model: EditBarModel;
}): React.JSX.Element | null {
  const [listOpen, setListOpen] = useState(false);
  const listId = useId();
  if (!model.uncertain && !model.count && !model.canRedo) return null;
  return (
    <>
      {listOpen && model.list(listId)}
      <div className={styles.editToolbar} aria-label="Table editing">
        <Button
          size="sm"
          variant="ghost"
          className={styles.editNote}
          aria-expanded={listOpen}
          aria-controls={listOpen ? listId : undefined}
          disabled={!model.count}
          onClick={() => {
            setListOpen(!listOpen);
          }}
        >
          <Icon name={listOpen ? 'chevronDown' : 'chevronRight'} />
          {model.note}
        </Button>
        <span className={styles.spacer} />
        <Button
          size="sm"
          variant="ghost"
          disabled={!model.canUndo || model.busy}
          onClick={model.undo}
        >
          Undo
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={!model.canRedo || model.busy}
          onClick={model.redo}
        >
          Redo
        </Button>
        <Button
          size="sm"
          disabled={!model.count || model.busy}
          onClick={model.discard}
        >
          {model.uncertain ? 'Reload & discard' : 'Discard'}
        </Button>
        <Button
          size="sm"
          variant="primary"
          disabled={!model.enabled || !model.count || model.busy}
          onClick={model.commit}
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
