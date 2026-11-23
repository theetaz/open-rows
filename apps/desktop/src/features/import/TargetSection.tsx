// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, SelectField, TextField } from '@openrows/ui';
import type { ImportDraft } from './model';
import styles from './Import.module.css';
import { t } from './messages';

const targetChoices = [
  { value: 'existing', label: t('existingTable') },
  { value: 'new', label: t('newTable') },
] as const;

/** Import into an existing table of the schema, or create one. */
export function TargetSection(props: {
  draft: ImportDraft;
  schema: string;
  tables: readonly string[];
  onChange: (patch: Partial<ImportDraft>) => void;
}): React.JSX.Element {
  const { draft, onChange } = props;
  const options = props.tables.includes(draft.table)
    ? props.tables
    : [draft.table, ...props.tables];
  return (
    <section className={styles.form} aria-label={t('target')}>
      <div className={styles.row}>
        <span className={styles.rowLabel} aria-hidden="true">
          {t('target')}
        </span>
        <SegmentedControl
          className={styles.segmented}
          size="compact"
          aria-label={t('target')}
          options={targetChoices}
          value={draft.target}
          onValueChange={(target) => {
            onChange({
              target,
              table: target === draft.target ? draft.table : '',
            });
          }}
        />
      </div>
      {draft.target === 'existing' && props.tables.length > 0 ? (
        <SelectField
          className={styles.row}
          label={t('tableSchema', { schema: props.schema })}
          value={draft.table}
          options={options.map((name) => ({
            value: name,
            label: name || t('chooseTable'),
          }))}
          onValueChange={(table) => {
            onChange({ table });
          }}
        />
      ) : (
        <TextField
          className={styles.row}
          label={
            draft.target === 'new'
              ? t('newTableSchema', { schema: props.schema })
              : t('tableSchema', { schema: props.schema })
          }
          value={draft.table}
          spellCheck={false}
          onChange={(event) => {
            onChange({ table: event.target.value });
          }}
        />
      )}
    </section>
  );
}
