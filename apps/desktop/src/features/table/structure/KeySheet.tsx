// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { SelectField, TextField } from '@openrows/ui';
import { StageSheet } from './StageSheet';
import type { Structure } from './useStructure';
import styles from './forms/Forms.module.css';

/** Add an ordered primary or unique key with the live DDL preview. */
export function KeySheet({
  structure,
  onClose,
}: {
  structure: Structure;
  onClose: () => void;
}): React.JSX.Element {
  const [kind, setKind] = useState<'addPrimaryKey' | 'addUnique'>('addUnique');
  const [name, setName] = useState('');
  const [columns, setColumns] = useState<string[]>([]);
  const problem = !columns.length ? 'Choose at least one key column.' : null;
  return (
    <StageSheet
      structure={structure}
      title="Add key constraint"
      problem={problem}
      op={
        problem
          ? null
          : {
              kind: 'stage',
              changes: [{ kind, key: { name: name.trim() || null, columns } }],
            }
      }
      stageLabel="Stage key"
      onClose={onClose}
    >
      <SelectField
        label="Constraint"
        value={kind}
        options={[
          { value: 'addUnique', label: 'Unique' },
          { value: 'addPrimaryKey', label: 'Primary key' },
        ]}
        onValueChange={setKind}
      />
      <TextField
        label="Name"
        value={name}
        placeholder="Automatic"
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      <fieldset className={styles.keys}>
        <legend>Columns, in key order</legend>
        {structure.rows
          .filter((row) => !row.dropped)
          .map((row) => (
            <label key={row.id} className={styles.checkRow}>
              <input
                type="checkbox"
                checked={columns.includes(row.name)}
                onChange={() => {
                  setColumns((chosen) =>
                    chosen.includes(row.name)
                      ? chosen.filter((c) => c !== row.name)
                      : [...chosen, row.name],
                  );
                }}
              />
              {row.name}
              {columns.includes(row.name) &&
                ` (${String(columns.indexOf(row.name) + 1)})`}
            </label>
          ))}
      </fieldset>
    </StageSheet>
  );
}
