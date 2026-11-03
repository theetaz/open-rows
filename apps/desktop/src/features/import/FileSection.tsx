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
import { t } from './messages';

/** The picked file and how it is parsed. */
export function FileSection(props: {
  file: PickedFile | null;
  options: ImportOptionsDto | null;
  onPick: () => void;
  onOptions: (options: ImportOptionsDto) => void;
}): React.JSX.Element {
  return (
    <section className={styles.section} aria-label={t('file')}>
      <div className={styles.fileRow}>
        <Button onClick={props.onPick}>
          {props.file ? t('chooseAnotherFile') : t('chooseFile')}
        </Button>
        <span className={styles.fileName}>
          {props.file?.fileName ?? t('csvTsvJson')}
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
        label={t('format')}
        value={options.format}
        options={formatChoices}
        onValueChange={(format) => {
          set({ format });
        }}
      />
      <SelectField
        label={t('encoding')}
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
        {t('emptyValuesNull')}
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
        label={t('delimiter')}
        value={options.delimiter}
        options={delimiters}
        onValueChange={(delimiter) => {
          set({ delimiter });
        }}
      />
      <SelectField
        label={t('quote')}
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
        {t('firstRowHolds')}
      </label>
    </>
  );
}
