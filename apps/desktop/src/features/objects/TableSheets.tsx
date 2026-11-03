// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import type { EngineDto, ObjectChangeDto } from '@openrows/ipc';
import { cascades } from '../table/structure/forms/dialect';
import { ObjectSheet } from './ObjectSheet';
import type { ChangeScope } from './useObjectChange';
import styles from '../table/structure/forms/Forms.module.css';
import { t } from './messages';

/** The table a sheet acts on. */
type TableRef = ChangeScope & { schema: string; name: string };

type Props = {
  table: TableRef;
  engine: EngineDto;
  onClose: () => void;
  onDone: (change: ObjectChangeDto) => void;
};

const target = (table: TableRef): { schema: string; name: string } => ({
  schema: table.schema,
  name: table.name,
});

/** Rename a table in its schema. */
export function RenameTableSheet({
  table,
  onClose,
  onDone,
}: Props): React.JSX.Element {
  const [name, setName] = useState(table.name);
  const changed = name.trim() !== '' && name.trim() !== table.name;
  return (
    <ObjectSheet
      title={t('renameTableName', { name: table.name })}
      scope={table}
      change={
        changed
          ? { kind: 'renameTable', target: target(table), name: name.trim() }
          : null
      }
      action={t('rename')}
      onClose={onClose}
      onDone={onDone}
    >
      <TextField
        label={t('newName')}
        value={name}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
    </ObjectSheet>
  );
}

/** A new table with the same structure, optionally with every row. */
export function DuplicateTableSheet({
  table,
  onClose,
  onDone,
}: Props): React.JSX.Element {
  const [name, setName] = useState(`${table.name}_copy`);
  const [withData, setWithData] = useState(false);
  const change: ObjectChangeDto | null = name.trim()
    ? {
        kind: 'duplicateTable',
        target: target(table),
        name: name.trim(),
        withData,
      }
    : null;
  return (
    <ObjectSheet
      title={t('duplicateTableName', { name: table.name })}
      scope={table}
      change={change}
      action={t('duplicate')}
      onClose={onClose}
      onDone={onDone}
    >
      <TextField
        label={t('nameCopy')}
        value={name}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      <label className={styles.checkRow}>
        <input
          type="checkbox"
          checked={withData}
          onChange={(event) => {
            setWithData(event.target.checked);
          }}
        />
        {t('copyRowsToo')}
      </label>
    </ObjectSheet>
  );
}

/** Remove every row of a table. */
export function TruncateTableSheet({
  table,
  engine,
  onClose,
  onDone,
}: Props): React.JSX.Element {
  const [cascade, setCascade] = useState(false);
  return (
    <ObjectSheet
      title={t('truncateTableName', { name: table.name })}
      scope={table}
      change={{ kind: 'truncateTable', target: target(table), cascade }}
      action={t('truncate')}
      destructive
      onClose={onClose}
      onDone={onDone}
    >
      <p>{t('everyRowName', { name: table.name })}</p>
      {cascades(engine) && (
        <label className={styles.checkRow}>
          <input
            type="checkbox"
            checked={cascade}
            onChange={(event) => {
              setCascade(event.target.checked);
            }}
          />
          {t('alsoTruncateTables')}
        </label>
      )}
    </ObjectSheet>
  );
}
