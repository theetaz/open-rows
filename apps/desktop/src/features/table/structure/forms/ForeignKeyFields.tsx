// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
import { SelectField, TextField } from '@openrows/ui';
import type { NewForeignKeyDto, ReferentialActionDto } from '@openrows/ipc';
import { referentialActions } from './dialect';
import { useReferenced } from './useReferenced';
import styles from './Forms.module.css';

/** What the foreign-key form edits. */
export type ForeignKeyDraft = NewForeignKeyDto;

/** A key from no columns to a table in `schema`. */
export function emptyForeignKey(schema: string): ForeignKeyDraft {
  return {
    name: null,
    columns: [],
    target: { schema, table: '', columns: [] },
    onDelete: 'noAction',
    onUpdate: 'noAction',
  };
}

/** Why the key cannot be staged yet, or `null`. */
export function foreignKeyProblem(draft: ForeignKeyDraft): string | null {
  if (!draft.columns.length) return 'Choose the referencing columns.';
  if (!draft.target.table.trim()) return 'Choose the referenced table.';
  if (draft.target.columns.length !== draft.columns.length)
    return 'Choose as many referenced columns as columns, in the same order.';
  return null;
}

/** The draft as sent: a blank name is none. */
export function cleanForeignKey(draft: ForeignKeyDraft): ForeignKeyDraft {
  return {
    ...draft,
    name: draft.name?.trim() || null,
    target: { ...draft.target, table: draft.target.table.trim() },
  };
}

/** Columns picked in click order, numbered. */
function Picks({
  legend,
  options,
  chosen,
  onChosen,
}: {
  legend: string;
  options: readonly string[];
  chosen: readonly string[];
  onChosen: (chosen: string[]) => void;
}): React.JSX.Element {
  return (
    <fieldset className={styles.keys}>
      <legend>{legend}</legend>
      {options.map((option) => {
        const position = chosen.indexOf(option);
        return (
          <label key={option} className={styles.checkRow}>
            <input
              type="checkbox"
              checked={position >= 0}
              onChange={() => {
                onChosen(
                  position >= 0
                    ? chosen.filter((item) => item !== option)
                    : [...chosen, option],
                );
              }}
            />
            {option}
            {position >= 0 && ` (${String(position + 1)})`}
          </label>
        );
      })}
    </fieldset>
  );
}

type FieldsProps = {
  draft: ForeignKeyDraft;
  onChange: (draft: ForeignKeyDraft) => void;
  /** Columns of the referencing table. */
  columns: readonly string[];
  scope: { profileId: string; database: string };
};

/** The referenced schema and table (the schema's tables are suggested). */
function TargetFields({
  draft,
  tables,
  onTarget,
}: {
  draft: ForeignKeyDraft;
  tables: readonly string[];
  onTarget: (patch: Partial<ForeignKeyDraft['target']>) => void;
}): React.JSX.Element {
  const list = useId();
  return (
    <>
      <TextField
        label="Referenced schema"
        value={draft.target.schema}
        onChange={(event) => {
          onTarget({ schema: event.target.value, table: '', columns: [] });
        }}
      />
      <TextField
        label="Referenced table"
        list={list}
        value={draft.target.table}
        autoComplete="off"
        onChange={(event) => {
          onTarget({ table: event.target.value, columns: [] });
        }}
      />
      <datalist id={list}>
        {tables.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
    </>
  );
}

/** `ON DELETE` and `ON UPDATE`. */
function Actions({
  draft,
  onChange,
}: Pick<FieldsProps, 'draft' | 'onChange'>): React.JSX.Element {
  const action = (
    label: string,
    field: 'onDelete' | 'onUpdate',
  ): React.JSX.Element => (
    <SelectField<ReferentialActionDto>
      label={label}
      value={draft[field]}
      options={referentialActions}
      onValueChange={(next) => {
        onChange({ ...draft, [field]: next });
      }}
    />
  );
  return (
    <>
      {action('On delete', 'onDelete')}
      {action('On update', 'onUpdate')}
    </>
  );
}

/**
 * Name, referencing columns, referenced schema, table and columns (the
 * table's primary key by default) and the `ON DELETE` / `ON UPDATE` actions.
 */
export function ForeignKeyFields(props: FieldsProps): React.JSX.Element {
  const { draft, onChange } = props;
  const setTarget = (patch: Partial<ForeignKeyDraft['target']>): void => {
    onChange({ ...draft, target: { ...draft.target, ...patch } });
  };
  const referenced = useReferenced(props.scope, draft.target, (key) => {
    if (key.length && !draft.target.columns.length) setTarget({ columns: key });
  });
  return (
    <div className={styles.grid}>
      <TextField
        label="Name"
        className={styles.wide}
        value={draft.name ?? ''}
        placeholder="Automatic"
        autoComplete="off"
        onChange={(event) => {
          onChange({ ...draft, name: event.target.value });
        }}
      />
      <TargetFields
        draft={draft}
        tables={referenced.tables}
        onTarget={setTarget}
      />
      <Picks
        legend="Columns"
        options={props.columns}
        chosen={draft.columns}
        onChosen={(chosen) => {
          onChange({ ...draft, columns: chosen });
        }}
      />
      <Picks
        legend="Referenced columns"
        options={referenced.columns}
        chosen={draft.target.columns}
        onChosen={(chosen) => {
          setTarget({ columns: chosen });
        }}
      />
      <Actions draft={draft} onChange={onChange} />
    </div>
  );
}
