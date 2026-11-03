// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { SelectField, TextField } from '@openrows/ui';
import { StageSheet } from './StageSheet';
import type { Structure } from './useStructure';
import styles from './forms/Forms.module.css';
import { t } from '../messages/lazyStructure';

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
  const problem = !columns.length ? t('chooseLeastOne') : null;
  return (
    <StageSheet
      structure={structure}
      title={t('addKeyConstraint')}
      problem={problem}
      op={
        problem
          ? null
          : {
              kind: 'stage',
              changes: [{ kind, key: { name: name.trim() || null, columns } }],
            }
      }
      stageLabel={t('stageKey')}
      onClose={onClose}
    >
      <SelectField
        label={t('constraint')}
        value={kind}
        options={[
          { value: 'addUnique', label: t('unique') },
          { value: 'addPrimaryKey', label: t('primaryKey') },
        ]}
        onValueChange={setKind}
      />
      <TextField
        label={t('name')}
        value={name}
        placeholder={t('automatic')}
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      <fieldset className={styles.keys}>
        <legend>{t('columnsKeyOrder')}</legend>
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
