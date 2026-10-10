// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import { ScopeList } from './ScopeList';
import type { DumpTableDto } from '@openrows/ipc';
import { sameTable } from './scopeModel';
import type { ScopeSchema } from './scopeModel';

/** PostgreSQL table definitions can be backed up without those tables' data. */
export function ExcludeTableData(props: {
  catalog: readonly ScopeSchema[];
  selected: readonly DumpTableDto[];
  onChange: (tables: DumpTableDto[]) => void;
}): React.JSX.Element {
  const [search, setSearch] = useState('');
  const tables = props.catalog
    .flatMap((schema) => schema.tables)
    .filter((table) =>
      `${table.schema}.${table.name}`
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    );
  return (
    <details>
      <summary>Exclude table data (PostgreSQL)</summary>
      <TextField
        aria-label="Search data exclusions"
        value={search}
        onChange={(event) => {
          setSearch(event.target.value);
        }}
      />
      <ScopeList
        key={search}
        rows={tables}
        render={(table) => (
          <label key={JSON.stringify(table)}>
            <input
              type="checkbox"
              checked={props.selected.some((item) => sameTable(item, table))}
              onChange={(event) => {
                const other = props.selected.filter(
                  (item) => !sameTable(item, table),
                );
                props.onChange(
                  event.target.checked ? [...other, table] : other,
                );
              }}
            />
            Exclude data for {table.schema}.{table.name}
          </label>
        )}
      />
    </details>
  );
}
