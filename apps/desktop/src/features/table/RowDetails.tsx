// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import { DetailsField } from './DetailsField';
import { matchingFields } from './row';
import type { SelectedRow } from './row';
import styles from './RowDetails.module.css';

/** Selected-row fields with search; values edit in place when the table is editable. */
export function RowDetails({ row }: { row: SelectedRow }): React.JSX.Element {
  const [search, setSearch] = useState('');
  const fields = matchingFields(row.fields, search);
  return (
    <div className={styles.details}>
      <TextField
        aria-label="Search for field"
        placeholder="Search for field…"
        type="search"
        value={search}
        onChange={(event) => {
          setSearch(event.target.value);
        }}
      />
      <small className={styles.caption}>
        {row.table} · row {row.number.toLocaleString('en-US')}
      </small>
      {row.error && <p role="alert">{row.error}</p>}
      <div role="list" aria-label="Row fields" className={styles.fields}>
        {fields.map((field) => (
          <div role="listitem" key={field.name}>
            <DetailsField field={field} editor={row.editor} />
          </div>
        ))}
      </div>
      {!fields.length && <p className={styles.note}>No matching fields</p>}
    </div>
  );
}
