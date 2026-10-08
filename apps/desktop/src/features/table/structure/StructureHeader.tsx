// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Icon, TextField } from '@openrows/ui';
import { tableValue } from './effective';
import type { Structure } from './useStructure';
import styles from './Structure.module.css';

/** `Name [table] · Primary [key chips] · Search for column…` above the grid. */
export function StructureHeader({
  structure,
  table,
}: {
  structure: Structure;
  table: string;
}): React.JSX.Element {
  const { schema } = structure;
  const staged = tableValue(schema.history.current, 'renameTable');
  const name = staged?.name ?? table;
  const primary = schema.detail?.primaryKey ?? [];
  return (
    <div className={styles.header}>
      <span>Name</span>
      <NameField
        key={name}
        name={name}
        enabled={schema.enabled}
        onRename={(next) => {
          schema.stage({ kind: 'renameTable', name: next });
        }}
      />
      <span>Primary</span>
      <span className={styles.primary} aria-label="Primary key">
        {primary.map((column) => (
          <span key={column} className={styles.chip}>
            {column}
          </span>
        ))}
      </span>
      {structure.mode === 'structure' && (
        <TextField
          className={styles.search}
          controlSize="compact"
          aria-label="Search for column"
          placeholder="Search for column…"
          leadingIcon={<Icon name="search" />}
          value={structure.search}
          onChange={(event) => {
            structure.setSearch(event.target.value);
          }}
        />
      )}
    </div>
  );
}

function NameField({
  name,
  enabled,
  onRename,
}: {
  name: string;
  enabled: boolean;
  onRename: (name: string) => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState(name);
  const commit = (): void => {
    if (draft.trim() && draft.trim() !== name) onRename(draft.trim());
    else setDraft(name);
  };
  return (
    <TextField
      className={styles.name}
      controlSize="compact"
      aria-label="Table name"
      value={draft}
      readOnly={!enabled}
      onChange={(event) => {
        setDraft(event.target.value);
      }}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === 'Enter') commit();
        if (event.key === 'Escape') setDraft(name);
      }}
    />
  );
}
