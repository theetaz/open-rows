// SPDX-License-Identifier: Apache-2.0
import { useId, useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import type { EngineDto, ObjectChangeDto } from '@openrows/ipc';
import {
  collationHints,
  encodingHints,
} from '../table/structure/forms/dialect';
import { ObjectSheet } from './ObjectSheet';
import type { ChangeScope } from './useObjectChange';
import { t } from './messages';

type Props = {
  scope: ChangeScope;
  engine: EngineDto;
  /** The database or schema to rename; `null` creates one. */
  name: string | null;
  onClose: () => void;
  onDone: (change: ObjectChangeDto) => void;
};

/** A database open in the app cannot be renamed until it is closed. */
type OpenDatabases = {
  opened: readonly string[];
  onCloseDatabase: (name: string) => void;
};

/** The create or rename request of a database form. */
function databaseChange(
  current: string | null,
  name: string,
  options: { encoding: string; collation: string },
): ObjectChangeDto | null {
  const next = name.trim();
  if (!next || next === current) return null;
  if (current !== null)
    return { kind: 'renameDatabase', name: current, to: next };
  return {
    kind: 'createDatabase',
    name: next,
    encoding: text(options.encoding),
    collation: text(options.collation),
  };
}

/** The create or rename request of a schema form. */
function schemaChange(
  current: string | null,
  name: string,
): ObjectChangeDto | null {
  const next = name.trim();
  if (!next || next === current) return null;
  return current === null
    ? { kind: 'createSchema', name: next }
    : { kind: 'renameSchema', name: current, to: next };
}

const text = (value: string): string | null => value.trim() || null;

/** Create a database (with encoding and collation) or rename one. */
export function DatabaseSheet(props: Props & OpenDatabases): React.JSX.Element {
  const [name, setName] = useState(props.name ?? '');
  const [encoding, setEncoding] = useState('');
  const [collation, setCollation] = useState('');
  const encodings = useId();
  const collations = useId();
  const mysql = props.engine === 'mysql' || props.engine === 'mariadb';
  const renamed = props.name !== null;
  const open = props.name !== null && props.opened.includes(props.name);
  const change = open
    ? null
    : databaseChange(props.name, name, { encoding, collation });
  return (
    <ObjectSheet
      title={
        renamed
          ? t('renameDatabaseName', { name: props.name ?? '' })
          : t('newDatabase')
      }
      scope={props.scope}
      change={change}
      problem={open ? t('nameOpenOpenrows', { name: props.name ?? '' }) : null}
      action={renamed ? t('rename') : t('create')}
      onClose={props.onClose}
      onDone={props.onDone}
    >
      {open && props.name !== null && (
        <Button
          onClick={() => {
            if (props.name !== null) props.onCloseDatabase(props.name);
          }}
        >
          {t('closeName', { name: props.name })}
        </Button>
      )}
      <TextField
        label={renamed ? t('newName') : t('name')}
        value={name}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      {!renamed && (
        <>
          <TextField
            label={mysql ? t('characterSet') : t('encoding')}
            list={encodings}
            placeholder={t('serverDefault')}
            value={encoding}
            onChange={(event) => {
              setEncoding(event.target.value);
            }}
          />
          <TextField
            label={t('collation')}
            list={collations}
            placeholder={t('serverDefault')}
            value={collation}
            onChange={(event) => {
              setCollation(event.target.value);
            }}
          />
          <datalist id={encodings}>
            {encodingHints(props.engine).map((item) => (
              <option key={item} value={item} />
            ))}
          </datalist>
          <datalist id={collations}>
            {collationHints(props.engine).map((item) => (
              <option key={item} value={item} />
            ))}
          </datalist>
        </>
      )}
    </ObjectSheet>
  );
}

/** Create a schema or rename one. */
export function SchemaSheet(props: Props): React.JSX.Element {
  const [name, setName] = useState(props.name ?? '');
  const renamed = props.name !== null;
  const change = schemaChange(props.name, name);
  return (
    <ObjectSheet
      title={
        renamed
          ? t('renameSchemaName', { name: props.name ?? '' })
          : t('newSchema')
      }
      scope={props.scope}
      change={change}
      action={renamed ? t('rename') : t('create')}
      onClose={props.onClose}
      onDone={props.onDone}
    >
      <TextField
        label={renamed ? t('newName') : t('name')}
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
