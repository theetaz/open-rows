// SPDX-License-Identifier: Apache-2.0
import { SelectField } from '@openrows/ui';
import type {
  BackupContentDto,
  BackupOptionsDto,
  DumpEngineDto,
} from '@openrows/ipc';
import { FORMATS } from './model';
import styles from './Dump.module.css';

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
    { key: 'noOwner', label: 'Leave out ownership (--no-owner)' },
    { key: 'noPrivileges', label: 'Leave out grants (--no-privileges)' },
  ],
  mysql: [
    {
      key: 'singleTransaction',
      label: 'One consistent snapshot (--single-transaction)',
    },
    { key: 'routines', label: 'Procedures and functions (--routines)' },
    { key: 'triggers', label: 'Triggers' },
    { key: 'events', label: 'Scheduled events (--events)' },
  ],
};

const CONTENTS: readonly { value: BackupContentDto; label: string }[] = [
  { value: 'all', label: 'Structure and data' },
  { value: 'schemaOnly', label: 'Structure only' },
  { value: 'dataOnly', label: 'Data only' },
];

const LEVELS = [
  { value: 'default', label: 'Tool default' },
  ...Array.from({ length: 10 }, (_, level) => ({
    value: String(level),
    label: level === 0 ? '0 (none)' : String(level),
  })),
];

function Compression(props: {
  options: BackupOptionsDto;
  onChange: (options: BackupOptionsDto) => void;
}): React.JSX.Element {
  const tar = props.options.format === 'tar';
  return (
    <SelectField
      label="Compression"
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
    <fieldset className={styles.options}>
      <legend>Options</legend>
      <div className={styles.row}>
        {postgres && (
          <SelectField
            label="Format"
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
          label="Content"
          value={options.content}
          options={CONTENTS}
          onValueChange={(content) => {
            onChange({ ...options, content });
          }}
        />
        {postgres && <Compression options={options} onChange={onChange} />}
      </div>
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
