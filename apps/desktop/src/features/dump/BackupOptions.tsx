// SPDX-License-Identifier: Apache-2.0
import { SelectField } from '@openrows/ui';
import type {
  BackupContentDto,
  BackupOptionsDto,
  DumpEngineDto,
} from '@openrows/ipc';
import { FORMATS } from './model';
import styles from './Dump.module.css';
import { t } from './messages';

type Flag =
  | 'noOwner'
  | 'noPrivileges'
  | 'singleTransaction'
  | 'routines'
  | 'triggers'
  | 'events';

const FLAGS: Readonly<
  Record<'postgres' | 'mysql', readonly { key: Flag; label: string }[]>
> = {
  postgres: [
    { key: 'noOwner', label: t('leaveOutOwnership') },
    { key: 'noPrivileges', label: t('leaveOutGrants') },
  ],
  mysql: [
    {
      key: 'singleTransaction',
      label: t('oneConsistentSnapshot'),
    },
    { key: 'routines', label: t('proceduresFunctionsRoutines') },
    { key: 'triggers', label: t('triggers') },
    { key: 'events', label: t('scheduledEventsEvents') },
  ],
};

const CONTENTS: readonly { value: BackupContentDto; label: string }[] = [
  { value: 'all', label: t('structureData') },
  { value: 'schemaOnly', label: t('structureOnly') },
  { value: 'dataOnly', label: t('dataOnly') },
];

const LEVELS = [
  { value: 'default', label: t('toolDefault') },
  ...Array.from({ length: 10 }, (_, level) => ({
    value: String(level),
    label: level === 0 ? t('n0None') : String(level),
  })),
];

function Compression(props: {
  options: BackupOptionsDto;
  onChange: (options: BackupOptionsDto) => void;
}): React.JSX.Element {
  const tar = props.options.format === 'tar';
  return (
    <SelectField
      className={styles.row}
      label={t('compression')}
      value={tar ? 'default' : String(props.options.compression ?? 'default')}
      disabled={tar}
      options={LEVELS}
      onValueChange={(value) => {
        const compression = value === 'default' ? null : Number(value);
        props.onChange({ ...props.options, compression });
      }}
    />
  );
}

/** The options a backup offers on its engine, as checkboxes and pickers. */
export function BackupOptions(props: {
  engine: DumpEngineDto;
  options: BackupOptionsDto;
  onChange: (options: BackupOptionsDto) => void;
}): React.JSX.Element | null {
  const { options, onChange } = props;
  if (props.engine === 'sqlite') return null;
  const postgres = props.engine === 'postgres';
  return (
    <fieldset className={styles.form}>
      <legend>{t('options')}</legend>
      {postgres && (
        <SelectField
          className={styles.row}
          label={t('format')}
          value={options.format}
          options={FORMATS}
          onValueChange={(format) => {
            const tar = format === 'tar';
            onChange({
              ...options,
              format,
              compression: tar ? null : options.compression,
            });
          }}
        />
      )}
      <SelectField
        className={styles.row}
        label={t('content')}
        value={options.content}
        options={CONTENTS}
        onValueChange={(content) => {
          onChange({ ...options, content });
        }}
      />
      {postgres && <Compression options={options} onChange={onChange} />}
      {FLAGS[props.engine].map((flag) => (
        <label key={flag.key}>
          <input
            type="checkbox"
            checked={options[flag.key]}
            onChange={(event) => {
              onChange({ ...options, [flag.key]: event.target.checked });
            }}
          />
          {flag.label}
        </label>
      ))}
    </fieldset>
  );
}
