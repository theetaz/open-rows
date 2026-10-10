// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import { pickConnectionFile } from '@openrows/ipc';
import type { PickFileKind } from '@openrows/ipc';
import { fieldError } from './form';
import { SwitchRow } from './SwitchRow';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

/**
 * A local database file (`SQLite`): typed, chosen in the native open panel,
 * or named in a save panel for a new file that is created on connect; or an
 * in-memory database, which hides the file controls. Only the path is
 * stored; the core opens the file.
 */
export function FileFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, update } = form;
  const duckdb = draft.engine === 'duckdb';
  const pick = (kind: PickFileKind, create: boolean): void => {
    // A panel that fails to open leaves the typed path; nothing to undo.
    pickConnectionFile(kind).then(
      (path) => {
        if (path) update({ filePath: path, fileCreate: create });
      },
      () => undefined,
    );
  };
  const memorySwitch = (
    <SwitchRow
      label="In-memory"
      detail={
        duckdb
          ? 'Shared by this connection’s tabs. Closing the connection or losing the driver process permanently loses all data.'
          : "A fresh database shared by this connection's tabs until it closes."
      }
      checked={draft.fileMemory}
      onChange={(fileMemory) => {
        update({ fileMemory });
      }}
    />
  );
  if (draft.fileMemory) return memorySwitch;
  return (
    <>
      {memorySwitch}
      <div className={styles.fileRow}>
        <TextField
          className={styles.row}
          label="Database file"
          value={draft.filePath}
          placeholder={`/path/to/database.${duckdb ? 'duckdb' : 'sqlite'}`}
          spellCheck={false}
          {...fieldError(errors.filePath)}
          onChange={(event) => {
            update({ filePath: event.target.value, fileCreate: false });
          }}
        />
        <Button
          aria-label="Choose database file"
          onClick={() => {
            pick(duckdb ? 'duckdbDatabase' : 'sqliteDatabase', false);
          }}
        >
          Choose…
        </Button>
        <Button
          aria-label="New database file"
          onClick={() => {
            pick(duckdb ? 'newDuckdbDatabase' : 'newSqliteDatabase', true);
          }}
        >
          New…
        </Button>
      </div>
      {draft.fileCreate && (
        <p className={styles.fileNote}>The file is created when you connect.</p>
      )}
      <SwitchRow
        label="Open read-only"
        detail={`${duckdb ? 'DuckDB' : 'SQLite'} refuses every write to this file.`}
        checked={draft.fileReadOnly}
        onChange={(fileReadOnly) => {
          update({ fileReadOnly });
        }}
      />
    </>
  );
}
