// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { matchingFields } from './row';
import type { SelectedRow } from './row';
import styles from './Table.module.css';

/** Compact selected-row fields with search, expandable values and eligible edit actions. */
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
      {row.error && <p role="alert">{row.error}</p>}
      <dl aria-label="Row fields">
        {fields.map((field) => (
          <div key={field.name}>
            <dt>
              <span>{field.name}</span>
              {row.onEdit && (
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`Edit field ${field.name}`}
                  onClick={() => {
                    row.onEdit?.(field.name);
                  }}
                >
                  Edit
                </Button>
              )}
              <small title={field.type ?? undefined}>{field.type}</small>
            </dt>
            <dd data-state={field.state}>
              {field.text.length > 100 || field.text.includes('\n') ? (
                <details>
                  <summary>{field.text.slice(0, 100)}</summary>
                  <pre>{field.text}</pre>
                </details>
              ) : (
                field.text
              )}
            </dd>
          </div>
        ))}
      </dl>
      {!fields.length && <p className={styles.note}>No matching fields</p>}
    </div>
  );
}
