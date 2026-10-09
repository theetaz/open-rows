// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { NULL_VALUE, textValue } from './cellValue';
import { matchingFields } from './row';
import type { BulkField, BulkRows } from './bulk/bulkRow';
import styles from './RowDetails.module.css';
import extras from './TableExtras.module.css';

/** The text a bulk field shows, and its placeholder when there is none. */
function shown(field: BulkField): { text: string; placeholder: string } {
  if (!('common' in field)) return { text: '', placeholder: 'Multiple values' };
  if (field.common === null) return { text: '', placeholder: 'NULL' };
  if (field.common === '') return { text: '', placeholder: 'EMPTY' };
  return { text: field.common ?? '', placeholder: '' };
}

/** A field's identity plus its shown value, so a new common value resets the editor. */
function fieldKey(field: BulkField): string {
  return JSON.stringify([field.name, 'common' in field ? field.common : 0]);
}

/** One field across the selected rows; Return or blur stages a typed value. */
function BulkFieldEditor({
  field,
  stage,
}: {
  field: BulkField;
  stage: BulkRows['stage'];
}): React.JSX.Element {
  const base = shown(field);
  const [text, setText] = useState(base.text);
  const commit = (): void => {
    if (stage && text !== base.text) stage(field.name, textValue(text));
  };
  return (
    <div
      className={styles.field}
      data-state={field.changed ? 'changed' : 'normal'}
    >
      <span className={styles.label}>
        <span>{field.name}</span>
        <small>{field.type ?? ''}</small>
      </span>
      <span className={extras.bulkValue}>
        <TextField
          className={styles.value}
          aria-label={field.name}
          placeholder={base.placeholder}
          readOnly={!stage}
          value={text}
          onChange={(event) => {
            setText(event.target.value);
          }}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
              event.preventDefault();
              commit();
            }
            if (event.key === 'Escape') setText(base.text);
          }}
        />
        <Button
          size="sm"
          variant="ghost"
          disabled={!stage}
          aria-label={`Set ${field.name} to NULL in every selected row`}
          onClick={() => {
            stage?.(field.name, NULL_VALUE);
          }}
        >
          NULL
        </Button>
      </span>
    </div>
  );
}

/**
 * Several selected rows in the inspector (FR-GRID-14): each field's common
 * value or "Multiple values"; a value entered here is staged in every
 * selected row and reviewed before commit like any other edit.
 */
export function BulkDetails({
  table,
  bulk,
  search,
}: {
  table: string;
  bulk: BulkRows;
  search: string;
}): React.JSX.Element {
  const fields = matchingFields(bulk.fields, search);
  return (
    <>
      <small className={styles.caption}>
        {table} · {bulk.rows.toLocaleString('en-US')} rows selected
        {bulk.stage
          ? ' · edits apply to every selected row and are reviewed before commit'
          : ' · read-only'}
      </small>
      <div
        role="list"
        aria-label="Fields of the selected rows"
        className={styles.fields}
      >
        {fields.map((field) => (
          <div role="listitem" key={fieldKey(field)}>
            <BulkFieldEditor field={field} stage={bulk.stage} />
          </div>
        ))}
      </div>
    </>
  );
}
