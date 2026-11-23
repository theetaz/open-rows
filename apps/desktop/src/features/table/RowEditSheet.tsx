// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { temporalKind } from './inline/temporal';
import { TemporalPicker } from './inline/TemporalPicker';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { EditInfoDto } from '@openrows/ipc';
import type { StagedEdit } from './editHistory';
import { editableText, fromOriginal, NULL_VALUE, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import type { RowEditing } from './useRowEditing';
import { duplicateRow, isDocumentColumn } from './quickEdits';
import { DocumentField } from './DocumentField';
import { invalidDocument } from './documentText';
import { ReviewSheet } from './ReviewSheet';
import { ReviewRows } from './ReviewRows';
import styles from './Table.module.css';
import { t } from './messages/lazyData';

/** A new document starts as `{}` so the JSON editor has an object to edit. */
function initialValues(draft: StagedEdit, info: EditInfoDto): Values {
  return draft.kind === 'insert' && info.columns.some(isDocumentColumn)
    ? { __document: textValue('{}'), ...draft.values }
    : draft.values;
}

/** Review precise text values; NULL and DEFAULT are explicit, different choices. */
function EditForm(props: {
  draft: StagedEdit;
  info: EditInfoDto;
  editing: RowEditing;
}): React.JSX.Element {
  const { draft, info, editing } = props;
  const [values, setValues] = useState(() => initialValues(draft, info));
  const set = (name: string, value: CellValue): void => {
    setValues((current) => ({ ...current, [name]: value }));
  };
  const inserting = draft.kind === 'insert';
  const invalid = invalidDocument(values.__document);
  return (
    <Sheet
      title={
        inserting
          ? t('addRow')
          : t('editRowNumber', { number: String(draft.number) })
      }
      onClose={editing.close}
    >
      <p className={styles.editNote}>{t('changesStagedUntil')}</p>
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
                {t('stageDeletion')}
              </Button>
              <Button
                variant="ghost"
                disabled={invalid}
                onClick={() => {
                  editing.change(duplicateRow({ ...draft, values }, info));
                }}
              >
                {t('stageDuplicate')}
              </Button>
            </>
          )
        }
      >
        <Button onClick={editing.close}>{t('cancel')}</Button>
        <Button
          variant="primary"
          disabled={invalid}
          onClick={() => {
            editing.change({ ...draft, values });
          }}
        >
          {t('stageChanges')}
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
        const document = isDocumentColumn(column);
        const protectedField = info.protected.includes(column.name);
        const active = Object.hasOwn(values, column.name);
        const value = shownValue(draft, values, column.name);
        return (
          <div key={column.name} className={styles.editField}>
            <label>
              <span>{column.name}</span>
              <small>{column.dataType}</small>
            </label>
            {inserting && !document && (
              <label>
                <input
                  type="checkbox"
                  aria-label={t('setName', { name: column.name })}
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
                {protectedField
                  ? t('useValueGenerated')
                  : t('useValueOtherwise')}
              </label>
            )}
            <ValueField
              column={column}
              value={value}
              focus={draft.focusField === column.name}
              disabled={
                protectedField ||
                (inserting && !active) ||
                value?.kind === 'null'
              }
              set={set}
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

/**
 * One column's value box, with a date/time picker for temporal types
 * (FR-DATA-06) and a validated JSON editor for a MongoDB document.
 */
function ValueField({
  column,
  value,
  focus,
  disabled,
  set,
}: {
  column: EditInfoDto['columns'][number];
  value: CellValue | undefined;
  focus: boolean;
  disabled: boolean;
  set: (name: string, value: CellValue) => void;
}): React.JSX.Element {
  const text = value ? editableText(value) : '';
  if (isDocumentColumn(column))
    return (
      <DocumentField
        value={text}
        disabled={disabled}
        onChange={(next) => {
          set(column.name, textValue(next));
        }}
      />
    );
  const kind = disabled ? null : temporalKind(column.dataType);
  return (
    <TextField
      aria-label={t('editName', { name: column.name })}
      data-initial-focus={focus}
      value={text}
      placeholder={placeholder(value)}
      disabled={disabled}
      onChange={(event) => {
        set(column.name, textValue(event.target.value));
      }}
      trailing={
        kind && (
          <TemporalPicker
            kind={kind}
            value={text}
            name={column.name}
            onPick={(picked) => {
              if (picked !== null) set(column.name, textValue(picked));
            }}
          />
        )
      }
    />
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
        title={t('reviewTableChanges')}
        summary={reviewSummary(editing)}
        warning={
          editing.info?.production ? t('productionConnectionThese') : undefined
        }
        items={
          <ReviewRows
            edits={editing.history.current}
            keyColumns={editing.info?.key ?? []}
          />
        }
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

/** Counts per operation; production is named in the sheet's banner. */
function reviewSummary(editing: RowEditing): string {
  const counts = { update: 0, insert: 0, delete: 0 };
  for (const change of editing.history.current) counts[change.kind] += 1;
  const parts = [
    counts.update && t('countUpdated', { count: String(counts.update) }),
    counts.insert && t('countInserted', { count: String(counts.insert) }),
    counts.delete && t('countDeleted', { count: String(counts.delete) }),
  ].filter((part): part is string => typeof part === 'string');
  return t('rowsSummary', { parts: parts.join(', ') });
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
        aria-label={t('nullName', { name })}
        data-initial-focus={focus && value?.kind === 'null'}
        checked={value?.kind === 'null'}
        onChange={(event) => {
          set(name, event.target.checked ? NULL_VALUE : textValue(''));
        }}
      />
      {t('null')}
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
