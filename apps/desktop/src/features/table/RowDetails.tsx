// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Icon, TextField } from '@openrows/ui';
import { BulkDetails } from './BulkDetails';
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
      <SearchField search={search} onSearch={setSearch} />
      {row.bulk ? (
        <BulkDetails table={row.table} bulk={row.bulk} search={search} />
      ) : (
        <SingleRow row={row} fields={fields} />
      )}
      {row.error && <p role="alert">{row.error}</p>}
    </div>
  );
}

/** Find a field in a wide row by name. */
function SearchField({
  search,
  onSearch,
}: {
  search: string;
  onSearch: (search: string) => void;
}): React.JSX.Element {
  return (
    <TextField
      aria-label="Search for field"
      placeholder="Search for field…"
      type="search"
      controlSize="search"
      leadingIcon={<Icon name="search" />}
      value={search}
      onChange={(event) => {
        onSearch(event.target.value);
      }}
    />
  );
}

/** One row's fields, editable in place when the table is editable. */
function SingleRow({
  row,
  fields,
}: {
  row: SelectedRow;
  fields: readonly SelectedRow['fields'][number][];
}): React.JSX.Element {
  return (
    <>
      <small className={styles.caption}>
        {row.table} · row {row.number.toLocaleString('en-US')}
      </small>
      <div role="list" aria-label="Row fields" className={styles.fields}>
        {fields.map((field) => (
          <div role="listitem" key={field.name}>
            <DetailsField
              field={field}
              editor={row.editor}
              source={row.source}
            />
          </div>
        ))}
      </div>
      {!fields.length && <p className={styles.note}>No matching fields</p>}
    </>
  );
}
