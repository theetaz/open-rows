// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { listObjects } from '@openrows/ipc';
import { ExportForm } from './ExportForm';
import { exportSettings } from './exportSettings';
import { startExportJob } from './exportJob';
import { formatDto, rememberedOptions, sheetOptions } from './options';
import styles from './Export.module.css';

/** The schema whose tables to export. */
export type TablesTarget = {
  sessionId: string;
  profileId: string;
  database: string;
  schema: string;
};

export type TablesExportSheetProps = TablesTarget & { onClose: () => void };

type Tables =
  | { status: 'loading' }
  | { status: 'ready'; names: readonly string[] }
  | { status: 'failed'; message: string };

const RELATIONS = [
  'table',
  'view',
  'materializedView',
  'foreignTable',
] as const;

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** The schema's tables and views from the catalog. */
function useTables(target: TablesTarget): Tables {
  const [tables, setTables] = useState<Tables>({ status: 'loading' });
  const { profileId, database, schema } = target;
  useEffect(() => {
    let live = true;
    listObjects(
      profileId,
      database,
      { schema, kinds: [...RELATIONS] },
      false,
    ).then(
      (reply) => {
        if (live)
          setTables({
            status: 'ready',
            names: reply.data.map((item) => item.name),
          });
      },
      (error: unknown) => {
        if (live) setTables({ status: 'failed', message: messageOf(error) });
      },
    );
    return () => {
      live = false;
    };
  }, [profileId, database, schema]);
  return tables;
}

/**
 * Export several tables of a schema (all of them by default): one file per
 * table into a folder, or one Excel workbook with a sheet per table.
 */
export function TablesExportSheet(
  props: TablesExportSheetProps,
): React.JSX.Element {
  const tables = useTables(props);
  const [excluded, setExcluded] = useState<ReadonlySet<string>>(new Set());
  const [workbook, setWorkbook] = useState(true);
  const [options, setOptions] = useState(() =>
    sheetOptions(exportSettings.get(), props.schema),
  );
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const chosen =
    tables.status === 'ready'
      ? tables.names.filter((name) => !excluded.has(name))
      : [];
  const format = formatDto(options);
  const run = (): void => {
    if (!format || chosen.length === 0) return;
    exportSettings.set(rememberedOptions(options));
    setStarting(true);
    setError(null);
    startExportJob({
      source: {
        kind: 'tables',
        sessionId: props.sessionId,
        target: {
          profileId: props.profileId,
          database: props.database,
          schema: props.schema,
        },
        tables: chosen,
        workbook: options.format === 'xlsx' && workbook,
      },
      format,
      fileName: props.schema,
    }).then(
      (started) => {
        setStarting(false);
        if (started) props.onClose();
      },
      (failure: unknown) => {
        setStarting(false);
        setError(messageOf(failure));
      },
    );
  };
  return (
    <Sheet title={`Export Tables of ${props.schema}`} onClose={props.onClose}>
      <TableChecklist
        tables={tables}
        excluded={excluded}
        onExcluded={setExcluded}
      />
      <ExportForm options={options} onOptions={setOptions} />
      <Layout
        xlsx={options.format === 'xlsx'}
        workbook={workbook}
        onWorkbook={setWorkbook}
      />
      {error && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!format || chosen.length === 0 || starting}
          onClick={run}
        >
          Export…
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function Layout(props: {
  xlsx: boolean;
  workbook: boolean;
  onWorkbook: (workbook: boolean) => void;
}): React.JSX.Element {
  if (!props.xlsx)
    return (
      <p className={styles.note}>
        Each table is written to its own file in a folder you choose.
      </p>
    );
  return (
    <fieldset className={styles.scope}>
      <legend>Files</legend>
      <label>
        <input
          type="radio"
          name="export-layout"
          checked={props.workbook}
          onChange={() => {
            props.onWorkbook(true);
          }}
        />
        One workbook with a sheet per table
      </label>
      <label>
        <input
          type="radio"
          name="export-layout"
          checked={!props.workbook}
          onChange={() => {
            props.onWorkbook(false);
          }}
        />
        One workbook per table in a folder
      </label>
    </fieldset>
  );
}

function TableChecklist(props: {
  tables: Tables;
  excluded: ReadonlySet<string>;
  onExcluded: (excluded: ReadonlySet<string>) => void;
}): React.JSX.Element {
  const { tables, excluded } = props;
  if (tables.status === 'loading')
    return (
      <p className={styles.note} role="status">
        Reading tables…
      </p>
    );
  if (tables.status === 'failed')
    return (
      <p className={styles.error} role="alert">
        {tables.message}
      </p>
    );
  const toggle = (name: string, on: boolean): void => {
    const next = new Set(excluded);
    if (on) next.delete(name);
    else next.add(name);
    props.onExcluded(next);
  };
  const count = tables.names.length - excluded.size;
  return (
    <>
      <div className={styles.tableTools}>
        <span>
          {count} of {tables.names.length} tables
        </span>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onExcluded(
              count === tables.names.length ? new Set(tables.names) : new Set(),
            );
          }}
        >
          {count === tables.names.length ? 'Select none' : 'Select all'}
        </Button>
      </div>
      <fieldset className={styles.tables} aria-label="Tables">
        {tables.names.map((name) => (
          <label key={name}>
            <input
              type="checkbox"
              checked={!excluded.has(name)}
              onChange={(event) => {
                toggle(name, event.target.checked);
              }}
            />
            {name}
          </label>
        ))}
      </fieldset>
    </>
  );
}
