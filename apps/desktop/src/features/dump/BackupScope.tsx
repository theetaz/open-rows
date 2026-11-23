// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon, TextField } from '@openrows/ui';
import { ScopeList } from './ScopeList';
import { sameTable } from './scopeModel';
import type { BackupSelection, ScopeSchema } from './scopeModel';
import type { DumpTableDto } from '@openrows/ipc';
import styles from './Dump.module.css';
import { t } from './messages';

type Props = {
  catalog: readonly ScopeSchema[];
  selection: BackupSelection;
  onChange: (selection: BackupSelection) => void;
};

/** Searchable schema and table choices, with explicit all/none controls. */
export function BackupScope(props: Props): React.JSX.Element {
  const [search, setSearch] = useState('');
  const { selection, onChange } = props;
  const match = (name: string): boolean =>
    name.toLocaleLowerCase().includes(search.toLocaleLowerCase());
  return (
    <fieldset className={styles.options}>
      <legend>{t('backupScope')}</legend>
      <label>
        <input
          type="checkbox"
          checked={selection.all}
          onChange={(event) => {
            onChange({ all: event.target.checked, schemas: [], tables: [] });
          }}
        />
        {t('wholeDatabase')}
      </label>
      <TextField
        aria-label={t('searchBackupObjects')}
        placeholder={t('searchBackupObjects')}
        controlSize="search"
        leadingIcon={<Icon name="search" />}
        value={search}
        onChange={(event) => {
          setSearch(event.target.value);
        }}
      />
      <div className={styles.buttons}>
        <Button
          size="sm"
          onClick={() => {
            onChange({
              all: false,
              schemas: props.catalog.map((schema) => schema.name),
              tables: [],
            });
          }}
        >
          {t('selectAll')}
        </Button>
        <Button
          size="sm"
          onClick={() => {
            onChange({ all: false, schemas: [], tables: [] });
          }}
        >
          {t('selectNone')}
        </Button>
      </div>
      <ScopeList
        key={search}
        columns
        rows={scopeRows(props.catalog, match)}
        render={(row) =>
          row.kind === 'schema' ? (
            <SchemaCheck key={JSON.stringify(row)} {...props} name={row.name} />
          ) : (
            <TableCheck
              key={JSON.stringify(row)}
              {...props}
              table={row.table}
            />
          )
        }
      />
      {!selection.all &&
        !selection.schemas.length &&
        !selection.tables.length && <p role="status">{t('selectLeastOne')}</p>}
      {!!selection.schemas.length && !!selection.tables.length && (
        <p className={styles.note}>{t('mixedSelectionsInclude')}</p>
      )}
    </fieldset>
  );
}

type ScopeRow =
  { kind: 'schema'; name: string } | { kind: 'table'; table: DumpTableDto };

function scopeRows(
  catalog: readonly ScopeSchema[],
  match: (name: string) => boolean,
): ScopeRow[] {
  return catalog.flatMap((schema) => {
    const tables = schema.tables.filter((table) =>
      match(`${table.schema}.${table.name}`),
    );
    return match(schema.name) || tables.length
      ? [
          { kind: 'schema' as const, name: schema.name },
          ...tables.map((table) => ({ kind: 'table' as const, table })),
        ]
      : [];
  });
}

function SchemaCheck(props: Props & { name: string }): React.JSX.Element {
  const { name, selection, onChange } = props;
  return (
    <label>
      <input
        type="checkbox"
        checked={selection.all || selection.schemas.includes(name)}
        disabled={selection.all}
        onChange={(event) => {
          const other = selection.schemas.filter((item) => item !== name);
          onChange({
            ...selection,
            schemas: event.target.checked ? [...other, name] : other,
          });
        }}
      />
      {t('schemaName', { name })}
    </label>
  );
}

function TableCheck(props: Props & { table: DumpTableDto }): React.JSX.Element {
  const { table, selection, onChange } = props;
  const selected = selection.all || selection.schemas.includes(table.schema);
  return (
    <label className={styles.scopeTable}>
      <input
        type="checkbox"
        checked={
          selected || selection.tables.some((item) => sameTable(item, table))
        }
        disabled={selected}
        onChange={(event) => {
          const other = selection.tables.filter(
            (item) => !sameTable(item, table),
          );
          onChange({
            ...selection,
            all: false,
            tables: event.target.checked ? [...other, table] : other,
          });
        }}
      />
      {t('tableSchemaName', { schema: table.schema, name: table.name })}
    </label>
  );
}
