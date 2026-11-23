// SPDX-License-Identifier: Apache-2.0
import { Button, EngineIcon, isEngineId, TextField } from '@openrows/ui';
import { pickConnectionFile } from '@openrows/ipc';
import type { PickFileKind } from '@openrows/ipc';
import { fieldError } from './form';
import { SwitchRow } from './SwitchRow';
import type { ConnectionForm } from './useConnectionForm';
import styles from './flow/Form.module.css';
import well from './FileFields.module.css';
import { t } from './messages/profile';

/**
 * The database file well (`SQLite`, DuckDB, libSQL file): typed, chosen in
 * the native open panel, or named in a save panel for a new file that is
 * created on connect. Only the path is stored; the core opens the file.
 */
export function FileFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element | null {
  const { draft, errors, update } = form;
  const duckdb = draft.engine === 'duckdb';
  if (draft.fileMemory) return null;
  const pick = (kind: PickFileKind, create: boolean): void => {
    // A panel that fails to open leaves the typed path; nothing to undo.
    pickConnectionFile(kind).then(
      (path) => {
        if (path) update({ filePath: path, fileCreate: create });
      },
      () => undefined,
    );
  };
  return (
    <div className={well.well}>
      {isEngineId(draft.engine) && (
        <EngineIcon engine={draft.engine} size="lg" />
      )}
      <div className={well.path}>
        <TextField
          className={styles.mono}
          label={t('databaseFile')}
          value={draft.filePath}
          placeholder={
            duckdb ? t('pathDatabaseDuckdb') : t('pathDatabaseSqlite')
          }
          spellCheck={false}
          {...fieldError(errors.filePath)}
          onChange={(event) => {
            update({ filePath: event.target.value, fileCreate: false });
          }}
        />
        {draft.fileCreate && <small>{t('fileCreatedWhen')}</small>}
      </div>
      <Button
        aria-label={t('chooseDatabaseFile')}
        onClick={() => {
          pick(duckdb ? 'duckdbDatabase' : 'sqliteDatabase', false);
        }}
      >
        {t('choose')}
      </Button>
      <Button
        aria-label={t('newDatabaseFile')}
        onClick={() => {
          pick(duckdb ? 'newDuckdbDatabase' : 'newSqliteDatabase', true);
        }}
      >
        {t('new')}
      </Button>
    </div>
  );
}

/** Access rows: open read-only, or an in-memory database instead of a file. */
export function FileAccessFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, update } = form;
  const duckdb = draft.engine === 'duckdb';
  return (
    <>
      {!draft.fileMemory && (
        <SwitchRow
          label={t('openReadOnly')}
          detail={duckdb ? t('duckdbRefusesEvery') : t('sqliteRefusesEvery')}
          checked={draft.fileReadOnly}
          onChange={(fileReadOnly) => {
            update({ fileReadOnly });
          }}
        />
      )}
      <SwitchRow
        label={t('memory')}
        detail={duckdb ? t('sharedConnectionS') : t('freshDatabaseShared')}
        checked={draft.fileMemory}
        onChange={(fileMemory) => {
          update({ fileMemory });
        }}
      />
    </>
  );
}
