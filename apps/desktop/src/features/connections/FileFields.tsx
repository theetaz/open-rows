// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import { pickConnectionFile } from '@openrows/ipc';
import type { PickFileKind } from '@openrows/ipc';
import { fieldError } from './form';
import { SwitchRow } from './SwitchRow';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/profile';

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
      label={t('memory')}
      detail={duckdb ? t('sharedConnectionS') : t('freshDatabaseShared')}
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
      {draft.fileCreate && (
        <p className={styles.fileNote}>{t('fileCreatedWhen')}</p>
      )}
      <SwitchRow
        label={t('openReadOnly')}
        detail={duckdb ? t('duckdbRefusesEvery') : t('sqliteRefusesEvery')}
        checked={draft.fileReadOnly}
        onChange={(fileReadOnly) => {
          update({ fileReadOnly });
        }}
      />
    </>
  );
}
