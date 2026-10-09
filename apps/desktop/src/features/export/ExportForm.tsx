// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import {
  compressionChoices,
  formatChoices,
  layoutChoices,
  sqlError,
} from './options';
import type { ExportOptions } from './options';
import { DelimitedFields } from './DelimitedFields';
import styles from './Export.module.css';

type Props = {
  options: ExportOptions;
  onOptions: (options: ExportOptions) => void;
};

export type Section = {
  options: ExportOptions;
  set: (patch: Partial<ExportOptions>) => void;
};

/** Format picker and the chosen format's options. */
export function ExportForm({ options, onOptions }: Props): React.JSX.Element {
  const set = (patch: Partial<ExportOptions>): void => {
    onOptions({ ...options, ...patch });
  };
  return (
    <fieldset className={styles.form}>
      <SelectField
        label="Format"
        value={options.format}
        options={formatChoices}
        onValueChange={(format) => {
          set({ format });
        }}
      />
      <FormatOptions options={options} set={set} />
    </fieldset>
  );
}

function FormatOptions({ options, set }: Section): React.JSX.Element {
  switch (options.format) {
    case 'csv':
    case 'tsv':
      return <DelimitedFields options={options} set={set} />;
    case 'json':
      return (
        <SelectField
          label="Layout"
          value={options.json.pretty ? 'pretty' : 'compact'}
          options={layoutChoices}
          onValueChange={(layout) => {
            set({ json: { pretty: layout === 'pretty' } });
          }}
        />
      );
    case 'sql':
      return <SqlOptions options={options} set={set} />;
    case 'xlsx':
      return <ExcelOptions options={options} set={set} />;
    case 'parquet':
      return (
        <>
          <SelectField
            label="Compression"
            value={options.parquet.compression}
            options={compressionChoices}
            onValueChange={(compression) => {
              set({ parquet: { compression } });
            }}
          />
          <p className={styles.note}>
            Columns keep their database types; intervals are written as text.
          </p>
        </>
      );
    case 'ndjson':
      return (
        <p className={styles.note}>
          One JSON object per line, for streaming tools.
        </p>
      );
    case 'markdown':
      return (
        <p className={styles.note}>
          A GitHub-flavoured table; NULL is written as NULL.
        </p>
      );
  }
}

function ExcelOptions({ options, set }: Section): React.JSX.Element {
  return (
    <>
      <label className={styles.checkRow}>
        <input
          type="checkbox"
          checked={options.xlsx.header}
          onChange={(event) => {
            set({ xlsx: { header: event.target.checked } });
          }}
        />
        Column names as a bold first row
      </label>
      <p className={styles.note}>
        Numbers, dates and booleans keep their types. A sheet holds 1,048,576
        rows; longer results continue on extra sheets.
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
        value={options.rowsText}
        inputMode="numeric"
        {...(error ? { error } : {})}
        onChange={(event) => {
          set({ rowsText: event.target.value });
        }}
      />
      <p className={styles.note}>
        Names and values are quoted for this connection&apos;s database.
      </p>
    </>
  );
}
