// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { EditInfoDto } from '@openrows/ipc';
import type { StagedEdit } from './editHistory';
import { editableText, fromOriginal, NULL_VALUE, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import type { RowEditing } from './useRowEditing';
import { duplicateRow } from './quickEdits';
import { ReviewSheet } from './ReviewSheet';
import styles from './Table.module.css';

/** Review precise text values; NULL and DEFAULT are explicit, different choices. */
function EditForm(props: {
  draft: StagedEdit;
  info: EditInfoDto;
  editing: RowEditing;
}): React.JSX.Element {
  const { draft, info, editing } = props;
  const [values, setValues] = useState(draft.values);
  const set = (name: string, value: CellValue): void => {
    setValues((current) => ({ ...current, [name]: value }));
  };
  const inserting = draft.kind === 'insert';
  return (
    <Sheet
      title={inserting ? 'Add row' : `Edit row ${String(draft.number)}`}
      onClose={editing.close}
    >
      <p className={styles.editNote}>
        Changes are staged until you review and commit them.
      </p>
      <EditFields
        draft={draft}
        info={info}
        values={values}
        set={set}
        setValues={setValues}
      />
      <SheetActions
        leading={
          !inserting && (
            <>
              <Button
                variant="ghost"
                onClick={() => {
                  editing.change({ ...draft, kind: 'delete', values: {} });
                }}
              >
                Stage deletion
              </Button>
              <Button
                variant="ghost"
                onClick={() => {
                  editing.change(duplicateRow({ ...draft, values }, info));
                }}
              >
                Stage duplicate
              </Button>
            </>
          )
        }
      >
        <Button onClick={editing.close}>Cancel</Button>
        <Button
          variant="primary"
          onClick={() => {
            editing.change({ ...draft, values });
          }}
        >
          Stage changes
        </Button>
      </SheetActions>
    </Sheet>
  );
}
type Values = StagedEdit['values'];
function EditFields({
  draft,
  info,
  values,
  set,
  setValues,
}: {
  draft: StagedEdit;
  info: EditInfoDto;
  values: Values;
  set: (name: string, value: CellValue) => void;
  setValues: React.Dispatch<React.SetStateAction<Values>>;
}): React.JSX.Element {
  const inserting = draft.kind === 'insert';
  return (
    <div className={styles.editFields}>
      {info.columns.map((column) => {
        const protectedField = info.protected.includes(column.name);
        const active = Object.hasOwn(values, column.name);
        const value = shownValue(draft, values, column.name);
        return (
          <div key={column.name} className={styles.editField}>
            <label>
              <span>{column.name}</span>
              <small>{column.dataType}</small>
            </label>
            {inserting && (
              <label>
                <input
                  type="checkbox"
                  aria-label={`Set ${column.name}`}
                  disabled={protectedField}
                  checked={active}
                  onChange={(event) => {
                    if (event.target.checked) set(column.name, textValue(''));
                    else
                      setValues((current) =>
                        Object.fromEntries(
                          Object.entries(current).filter(
                            ([name]) => name !== column.name,
                          ),
                        ),
                      );
                  }}
                />
                Use a value{' '}
                {protectedField ? '(generated)' : '(otherwise DEFAULT)'}
              </label>
            )}
            <TextField
              aria-label={`Edit ${column.name}`}
              data-initial-focus={draft.focusField === column.name}
              value={value ? editableText(value) : ''}
              placeholder={placeholder(value)}
              disabled={
                protectedField ||
                (inserting && !active) ||
                value?.kind === 'null'
              }
              onChange={(event) => {
                set(column.name, textValue(event.target.value));
              }}
            />
            {column.nullable && !protectedField && (!inserting || active) && (
              <NullValue
                name={column.name}
                value={value}
                set={set}
                focus={draft.focusField === column.name}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

/** Value editor or transactional commit preview. */
export function RowEditSheet({
  editing,
}: {
  editing: RowEditing;
}): React.JSX.Element | null {
  if (editing.review)
    return (
      <ReviewSheet
        title="Review table changes"
        summary={reviewSummary(editing)}
        sql={editing.review.statements
          .map(
            (statement) =>
              `${statement.sql}\n${statement.params.map((value, index) => `-- $${String(index + 1)} = ${value === null ? 'NULL' : JSON.stringify(value)}`).join('\n')}`,
          )
          .join('\n\n')}
        error={editing.error}
        busy={editing.busy}
        blocked={Boolean(editing.history.uncertain)}
        onCancel={editing.close}
        onCommit={() => {
          editing.commit(editing.review?.token);
        }}
      />
    );
  return editing.draft && editing.info ? (
    <EditForm
      key={editing.draft.id}
      draft={editing.draft}
      info={editing.info}
      editing={editing}
    />
  ) : null;
}

/** Counts per operation, with the environment named when the connection is production. */
function reviewSummary(editing: RowEditing): string {
  const counts = { update: 0, insert: 0, delete: 0 };
  for (const change of editing.history.current) counts[change.kind] += 1;
  const parts = [
    counts.update && `${String(counts.update)} updated`,
    counts.insert && `${String(counts.insert)} inserted`,
    counts.delete && `${String(counts.delete)} deleted`,
  ].filter((part): part is string => typeof part === 'string');
  const where = editing.info?.production
    ? 'Production connection: these statements change production data. '
    : '';
  return `${where}Rows: ${parts.join(', ')}. One transaction: conflicts and statement errors roll everything back. If the connection is lost during commit, reload before retrying.`;
}

function NullValue({
  name,
  value,
  set,
  focus,
}: {
  name: string;
  value: CellValue | undefined;
  set: (name: string, value: CellValue) => void;
  focus: boolean;
}): React.JSX.Element {
  return (
    <label>
      <input
        type="checkbox"
        aria-label={`NULL ${name}`}
        data-initial-focus={focus && value?.kind === 'null'}
        checked={value?.kind === 'null'}
        onChange={(event) => {
          set(name, event.target.checked ? NULL_VALUE : textValue(''));
        }}
      />
      NULL
    </label>
  );
}

/** Markers for values without editable text; inactive insert fields use DEFAULT. */
function placeholder(value: CellValue | undefined): string {
  if (value?.kind === 'null') return 'NULL';
  return !value || value.kind === 'default' ? 'DEFAULT' : '';
}

/** The staged value, else the original (none for an insert's DEFAULT fields). */
function shownValue(
  draft: StagedEdit,
  values: StagedEdit['values'],
  name: string,
): CellValue | undefined {
  if (Object.hasOwn(values, name)) return values[name];
  return draft.kind === 'insert'
    ? undefined
    : fromOriginal(draft.original[name] ?? null);
}
