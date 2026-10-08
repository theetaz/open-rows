// SPDX-License-Identifier: Apache-2.0
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { ResultMeta } from '../query/run';
import { textValue } from './cellValue';
import type { CellValue } from './cellValue';
import type { StagedEdit } from './editHistory';
import { canEdit } from './EditToolbar';
import { fileEncoding } from './fieldActions';
import { readRow } from './row';
import type { RowEditor, RowField, SelectedRow, ShowRow } from './row';
import { stagedEditFor, withStaged } from './selectedRow';
import type { RowEditing } from './useRowEditing';

/** The focused page row, its display fields and (once loaded) its exact snapshot. */
type Picked = {
  ticket: number;
  result: ResultMeta;
  row: number;
  number: number;
  fields: readonly RowField[];
  error?: string;
  draft?: StagedEdit;
};
type Latest = { editing: RowEditing; picked: Picked | null };

/**
 * Publish the focused row to the inspector, and publish it again with staged
 * values whenever edits are staged, undone or discarded (without stealing
 * the inspector from another panel). Returns the grid's row-focus handler.
 */
export function useSelectedRow(args: {
  table: string;
  offset: number;
  editing: RowEditing;
  ready: boolean;
  onRow: ShowRow;
}): (result: ResultMeta, row: number) => void {
  const [picked, setPicked] = useState<Picked | null>(null);
  const sequence = useRef(0);
  const shown = useRef(0);
  const latest = useRef<Latest>({ editing: args.editing, picked });
  useEffect(() => {
    latest.current = { editing: args.editing, picked };
  });
  const writable = canEdit(args.editing, args.ready);
  const publish = useEffectEvent((current: Picked) => {
    const { editing } = args;
    const edit = stagedEditFor(
      current.fields,
      editing.history.current,
      editing.info?.key ?? [],
    );
    const row: SelectedRow = {
      table: args.table,
      number: current.number,
      fields: withStaged(current.fields, edit, current.draft?.original ?? null),
    };
    if (current.error) row.error = current.error;
    else if (writable && editing.info)
      row.editor = rowEditor(latest, setPicked, {
        info: editing.info,
        ready: Boolean(edit ?? current.draft),
      });
    args.onRow(row, shown.current !== current.ticket);
    shown.current = current.ticket;
  });
  const changes = args.editing.history.current;
  const info = args.editing.info;
  useEffect(() => {
    if (picked) publish(picked);
  }, [picked, changes, info, writable]);
  return (result, row) => {
    const ticket = ++sequence.current;
    const base = { ticket, result, row, number: args.offset + row + 1 };
    readRow(result, row).then(
      (fields) => {
        if (ticket === sequence.current) setPicked({ ...base, fields });
      },
      (error: unknown) => {
        if (ticket === sequence.current)
          setPicked({
            ...base,
            fields: [],
            error: error instanceof Error ? error.message : String(error),
          });
      },
    );
  };
}

/** Inspector actions; they read the latest editing state when invoked. */
function rowEditor(
  latest: React.RefObject<Latest>,
  setPicked: React.Dispatch<React.SetStateAction<Picked | null>>,
  facts: { info: NonNullable<RowEditing['info']>; ready: boolean },
): RowEditor {
  const draftOf = (): StagedEdit | undefined => {
    const { editing, picked } = latest.current;
    if (!picked) return undefined;
    const keys = editing.info?.key ?? [];
    return (
      stagedEditFor(picked.fields, editing.history.current, keys) ??
      picked.draft
    );
  };
  const stage = (name: string, value: CellValue): void => {
    const draft = draftOf();
    if (draft) latest.current.editing.stageField(draft, name, value);
  };
  return {
    columns: facts.info.columns,
    protected: facts.info.protected,
    ready: facts.ready,
    load: () => {
      const { editing, picked } = latest.current;
      if (!picked || draftOf()) return;
      editing.prepare(picked.result, picked.row, (draft) => {
        setPicked((current) =>
          current?.ticket === picked.ticket ? { ...current, draft } : current,
        );
      });
    },
    stage,
    addFile: (name) => {
      const column = facts.info.columns.find((item) => item.name === name);
      const encoding = column ? fileEncoding(column.dataType) : null;
      if (!encoding || !draftOf()) return;
      latest.current.editing.pickFile(encoding, (text) => {
        stage(name, textValue(text));
      });
    },
  };
}
