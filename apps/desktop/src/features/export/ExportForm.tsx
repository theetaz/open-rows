// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, SelectField, TextField } from '@openrows/ui';
import {
  delimiterChoices,
  formatChoices,
  layoutChoices,
  sqlError,
} from './options';
import type { ExportOptions } from './options';
import styles from './Export.module.css';

type Props = {
  options: ExportOptions;
  onOptions: (options: ExportOptions) => void;
};

/** Format picker and the chosen format's options. */
export function ExportForm({ options, onOptions }: Props): React.JSX.Element {
  const set = (patch: Partial<ExportOptions>): void => {
    onOptions({ ...options, ...patch });
  };
  return (
    <fieldset className={styles.form}>
      <SegmentedControl
        aria-label="Format"
        options={formatChoices}
        value={options.format}
        onValueChange={(format) => {
          set({ format });
        }}
      />
      {options.format === 'csv' && <CsvOptions options={options} set={set} />}
      {options.format === 'json' && (
        <SelectField
          label="Layout"
          value={options.lines ? 'lines' : 'array'}
          options={layoutChoices}
          onValueChange={(layout) => {
            set({ lines: layout === 'lines' });
          }}
        />
      )}
      {options.format === 'sql' && <SqlOptions options={options} set={set} />}
    </fieldset>
  );
}

type Section = {
  options: ExportOptions;
  set: (patch: Partial<ExportOptions>) => void;
};

function CsvOptions({ options, set }: Section): React.JSX.Element {
  return (
    <>
      <SelectField
        label="Delimiter"
        value={options.delimiter}
        options={delimiterChoices}
        onValueChange={(delimiter) => {
          set({ delimiter });
        }}
      />
      <label className={styles.checkRow}>
        <input
          type="checkbox"
          checked={options.header}
          onChange={(event) => {
            set({ header: event.target.checked });
          }}
        />
        Column names as the first row
      </label>
      <p className={styles.note}>
        RFC 4180 quoting; NULL is an empty field and an empty string is "".
      </p>
    </>
  );
}

function SqlOptions({ options, set }: Section): React.JSX.Element {
  const error = sqlError(options);
  return (
    <>
      <TextField
        label="Table name"
        value={options.table}
        spellCheck={false}
        onChange={(event) => {
          set({ table: event.target.value });
        }}
      />
      <TextField
        label="Rows per INSERT"
        value={options.rowsPerStatement}
        inputMode="numeric"
        {...(error ? { error } : {})}
        onChange={(event) => {
          set({ rowsPerStatement: event.target.value });
        }}
      />
      <p className={styles.note}>
        Names and values are quoted for this connection&apos;s database.
      </p>
    </>
  );
}
