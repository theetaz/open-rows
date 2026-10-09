// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { EngineDto, ObjectFeaturesDto } from '@openrows/ipc';
import { ColumnFields } from '../table/structure/forms/ColumnFields';
import { columnType, emptyColumn } from '../table/structure/forms/columnDraft';
import type { ColumnDraft } from '../table/structure/forms/columnDraft';
import type { TableDraft } from './newTable';
import { renameColumn } from './newTable';
import styles from '../table/structure/forms/Forms.module.css';

type Props = {
  draft: TableDraft;
  onChange: (draft: TableDraft) => void;
  engine: EngineDto;
  features: ObjectFeaturesDto;
};

/** A free `column_N` name. */
function freeName(draft: TableDraft): string {
  const names = new Set(draft.columns.map((column) => column.name));
  let index = draft.columns.length + 1;
  while (names.has(`column_${String(index)}`)) index += 1;
  return `column_${String(index)}`;
}

/** One column line: select it, toggle it in the primary key, remove it. */
function ColumnItem(props: {
  column: ColumnDraft;
  selected: boolean;
  keyed: boolean;
  removable: boolean;
  onSelect: () => void;
  onKey: () => void;
  onRemove: () => void;
}): React.JSX.Element {
  const { column } = props;
  return (
    <div
      role="option"
      aria-selected={props.selected}
      tabIndex={0}
      className={styles.item}
      onClick={props.onSelect}
      onKeyDown={(event) => {
        if (event.target === event.currentTarget && event.key === 'Enter')
          props.onSelect();
      }}
    >
      <span>
        {column.name || '(unnamed)'} {columnType(column)}
        {column.nullable ? '' : ' NOT NULL'}
      </span>
      <label className={styles.checkRow}>
        <input
          type="checkbox"
          aria-label={`Primary key: ${column.name}`}
          checked={props.keyed}
          onChange={props.onKey}
        />
        Key
      </label>
      <Button
        size="sm"
        aria-label={`Remove column ${column.name}`}
        disabled={!props.removable}
        onClick={(event) => {
          event.stopPropagation();
          props.onRemove();
        }}
      >
        Remove
      </Button>
    </div>
  );
}

/** Edits of the column list: key toggles, removals and the selected form. */
function columnEdits(
  props: Pick<Props, 'draft' | 'onChange'>,
  [selected, setSelected]: [number, (index: number) => void],
): {
  toggleKey: (name: string) => void;
  remove: (index: number, name: string) => void;
  edit: (next: ColumnDraft) => void;
} {
  const { draft, onChange } = props;
  return {
    toggleKey: (name) => {
      const keyed = draft.primaryKey.includes(name);
      onChange({
        ...draft,
        primaryKey: keyed
          ? draft.primaryKey.filter((item) => item !== name)
          : [...draft.primaryKey, name],
      });
    },
    remove: (index, name) => {
      onChange({
        ...draft,
        columns: draft.columns.filter((_, at) => at !== index),
        primaryKey: draft.primaryKey.filter((item) => item !== name),
      });
      setSelected(Math.max(0, Math.min(selected, draft.columns.length - 2)));
    },
    edit: (next) => {
      const column = draft.columns[selected];
      if (!column) return;
      const renamed =
        next.name !== column.name
          ? renameColumn(draft, column.name, next.name)
          : draft;
      onChange({
        ...renamed,
        columns: draft.columns.map((item, index) =>
          index === selected ? next : item,
        ),
      });
    },
  };
}

/**
 * The new table's columns: one line each with its primary-key toggle,
 * and the column form (the Structure view's) for the selected one.
 */
export function ColumnsSection(props: Props): React.JSX.Element {
  const { draft, onChange, engine, features } = props;
  const [selected, setSelected] = useState(0);
  const column = draft.columns[selected];
  const edits = columnEdits(props, [selected, setSelected]);
  return (
    <section className={styles.section} aria-label="Columns">
      <div className={styles.sectionHead}>
        <h3>Columns</h3>
        <Button
          size="sm"
          onClick={() => {
            onChange({
              ...draft,
              columns: [...draft.columns, emptyColumn(freeName(draft), engine)],
            });
            setSelected(draft.columns.length);
          }}
        >
          Add column
        </Button>
      </div>
      <div className={styles.list} role="listbox" aria-label="Table columns">
        {draft.columns.map((item, index) => (
          <ColumnItem
            key={index}
            column={item}
            selected={index === selected}
            keyed={draft.primaryKey.includes(item.name)}
            removable={draft.columns.length > 1}
            onSelect={() => {
              setSelected(index);
            }}
            onKey={() => {
              edits.toggleKey(item.name);
            }}
            onRemove={() => {
              edits.remove(index, item.name);
            }}
          />
        ))}
      </div>
      {column && (
        <ColumnFields
          draft={column}
          engine={engine}
          features={features}
          onChange={edits.edit}
        />
      )}
    </section>
  );
}
