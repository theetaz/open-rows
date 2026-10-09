// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField } from '@openrows/ui';
import type { ImportOptionsDto } from '@openrows/ipc';
import {
  delimiterChoices,
  encodingChoices,
  formatChoices,
  quoteChoices,
} from './choices';
import type { PickedFile } from './model';
import styles from './Import.module.css';

/** The picked file and how it is parsed. */
export function FileSection(props: {
  file: PickedFile | null;
  options: ImportOptionsDto | null;
  onPick: () => void;
  onOptions: (options: ImportOptionsDto) => void;
}): React.JSX.Element {
  return (
    <section className={styles.section} aria-label="File">
      <div className={styles.fileRow}>
        <Button onClick={props.onPick}>
          {props.file ? 'Choose Another File…' : 'Choose File…'}
        </Button>
        <span className={styles.fileName}>
          {props.file?.fileName ?? 'CSV, TSV, JSON or NDJSON'}
        </span>
      </div>
      {props.options && (
        <ParseOptions options={props.options} onOptions={props.onOptions} />
      )}
    </section>
  );
}

function ParseOptions(props: {
  options: ImportOptionsDto;
  onOptions: (options: ImportOptionsDto) => void;
}): React.JSX.Element {
  const { options } = props;
  const set = (patch: Partial<ImportOptionsDto>): void => {
    props.onOptions({ ...options, ...patch });
  };
  const delimited = options.format === 'delimited';
  const encodings = encodingChoices.some((c) => c.value === options.encoding)
    ? encodingChoices
    : [
        ...encodingChoices,
        { value: options.encoding, label: options.encoding },
      ];
  return (
    <div className={styles.optionGrid}>
      <SelectField
        label="Format"
        value={options.format}
        options={formatChoices}
        onValueChange={(format) => {
          set({ format });
        }}
      />
      <SelectField
        label="Encoding"
        value={options.encoding}
        options={encodings}
        onValueChange={(encoding) => {
          set({ encoding });
        }}
      />
      {delimited && <DelimitedOptions options={options} set={set} />}
      <label className={styles.checkRow}>
        <input
          type="checkbox"
          checked={options.emptyAsNull}
          onChange={(event) => {
            set({ emptyAsNull: event.target.checked });
          }}
        />
        Empty values are NULL
      </label>
    </div>
  );
}

function DelimitedOptions(props: {
  options: ImportOptionsDto;
  set: (patch: Partial<ImportOptionsDto>) => void;
}): React.JSX.Element {
  const { options, set } = props;
  const delimiters = delimiterChoices.some((c) => c.value === options.delimiter)
    ? delimiterChoices
    : [
        ...delimiterChoices,
        { value: options.delimiter, label: options.delimiter },
      ];
  return (
    <>
      <SelectField
        label="Delimiter"
        value={options.delimiter}
        options={delimiters}
        onValueChange={(delimiter) => {
          set({ delimiter });
        }}
      />
      <SelectField
        label="Quote"
        value={options.quote ?? 'none'}
        options={quoteChoices}
        onValueChange={(quote) => {
          set({ quote: quote === 'none' ? null : quote });
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
        First row holds column names
      </label>
    </>
  );
}
