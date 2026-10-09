// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import { badRowChoices, conflictChoices, transactionChoices } from './choices';
import type { ImportDraft } from './model';
import styles from './Import.module.css';

/** Conflicts, truncation, transactions, bad rows and batch size. */
export function LoadOptions(props: {
  draft: ImportDraft;
  onChange: (patch: Partial<ImportDraft>) => void;
}): React.JSX.Element {
  const { draft, onChange } = props;
  const existing = draft.target === 'existing';
  return (
    <section className={styles.optionGrid} aria-label="Import options">
      {existing && (
        <SelectField
          label="When a key already exists"
          value={draft.conflict}
          options={conflictChoices}
          onValueChange={(conflict) => {
            onChange({ conflict });
          }}
        />
      )}
      <SelectField
        label="Transaction"
        value={draft.transaction}
        options={transactionChoices}
        onValueChange={(transaction) => {
          onChange({ transaction });
        }}
      />
      <SelectField
        label="Rows that cannot be read"
        value={draft.badRows}
        options={badRowChoices}
        onValueChange={(badRows) => {
          onChange({ badRows });
        }}
      />
      <TextField
        label="Rows per batch"
        value={draft.batchSize}
        inputMode="numeric"
        onChange={(event) => {
          onChange({ batchSize: event.target.value });
        }}
      />
      {existing && (
        <label className={styles.checkRow}>
          <input
            type="checkbox"
            checked={draft.truncate}
            onChange={(event) => {
              onChange({ truncate: event.target.checked });
            }}
          />
          Delete all rows of the table first
        </label>
      )}
    </section>
  );
}
