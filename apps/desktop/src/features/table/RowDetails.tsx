// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import { matchingFields } from './row';
import type { SelectedRow } from './row';
import styles from './Table.module.css';

/** The selected table row's fields, vertically, with a field search (read-only until D-09). */
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
      <small>
        {row.table} · row {row.number.toLocaleString('en-US')}
      </small>
      <dl aria-label="Row fields">
        {fields.map((field) => (
          <div key={field.name}>
            <dt title={field.type ?? undefined}>{field.name}</dt>
            <dd data-state={field.state}>{field.text}</dd>
          </div>
        ))}
      </dl>
      {!fields.length && <p className={styles.note}>No matching fields</p>}
    </div>
  );
}
