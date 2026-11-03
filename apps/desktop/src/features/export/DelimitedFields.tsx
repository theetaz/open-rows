// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { Delimited } from './exportSettings';
import {
  delimiterChoices,
  encodingChoices,
  lineEndingChoices,
  nullTextError,
  quoteChoices,
} from './options';
import type { Section } from './ExportForm';
import styles from './Export.module.css';
import { t } from './lazyMessages';

/** CSV and TSV options: delimiter (CSV), quote, header, NULL text, line ends, encoding. */
export function DelimitedFields({ options, set }: Section): React.JSX.Element {
  const csv = options.format === 'csv';
  const text: Delimited = csv ? options.csv : options.tsv;
  const change = (patch: Partial<Delimited>): void => {
    if (csv) set({ csv: { ...options.csv, ...patch } });
    else set({ tsv: { ...options.tsv, ...patch } });
  };
  const error = nullTextError(options);
  return (
    <>
      {csv && (
        <SelectField
          label={t('delimiter')}
          value={options.csv.delimiter}
          options={delimiterChoices}
          onValueChange={(delimiter) => {
            set({ csv: { ...options.csv, delimiter } });
          }}
        />
      )}
      <div className={styles.pair}>
        <SelectField
          label={t('quote')}
          value={text.quote}
          options={quoteChoices}
          onValueChange={(quote) => {
            change({ quote });
          }}
        />
        <TextField
          label={t('null')}
          value={text.nullText}
          placeholder={t('emptyField')}
          spellCheck={false}
          {...(error ? { error } : {})}
          onChange={(event) => {
            change({ nullText: event.target.value });
          }}
        />
      </div>
      <div className={styles.pair}>
        <SelectField
          label={t('lineEndings')}
          value={text.lineEnding}
          options={lineEndingChoices}
          onValueChange={(lineEnding) => {
            change({ lineEnding });
          }}
        />
        <SelectField
          label={t('encoding')}
          value={text.encoding}
          options={encodingChoices}
          onValueChange={(encoding) => {
            change({ encoding });
          }}
        />
      </div>
      <label className={styles.checkRow}>
        <input
          type="checkbox"
          checked={text.header}
          onChange={(event) => {
            change({ header: event.target.checked });
          }}
        />
        {t('columnNamesFirst')}
      </label>
    </>
  );
}
