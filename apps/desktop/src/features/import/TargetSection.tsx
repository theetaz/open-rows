// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, SelectField, TextField } from '@openrows/ui';
import type { ImportDraft } from './model';
import styles from './Import.module.css';

const targetChoices = [
  { value: 'existing', label: 'Existing table' },
  { value: 'new', label: 'New table' },
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
    <section className={styles.section} aria-label="Target">
      <SegmentedControl
        aria-label="Target"
        options={targetChoices}
        value={draft.target}
        onValueChange={(target) => {
          onChange({
            target,
            table: target === draft.target ? draft.table : '',
          });
        }}
      />
      {draft.target === 'existing' && props.tables.length > 0 ? (
        <SelectField
          label={`Table in ${props.schema}`}
          value={draft.table}
          options={options.map((name) => ({
            value: name,
            label: name || 'Choose a table…',
          }))}
          onValueChange={(table) => {
            onChange({ table });
          }}
        />
      ) : (
        <TextField
          label={
            draft.target === 'new'
              ? `New table in ${props.schema}`
              : `Table in ${props.schema}`
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
