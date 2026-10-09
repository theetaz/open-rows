// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, IconButton, SelectField, TextField } from '@openrows/ui';
import { useSavedFilters } from '../presentationStore';
import type { SavedFilter } from '../presentation';
import type { TableView } from '../model';
import styles from '../TableExtras.module.css';

type Props = {
  /** The table's presentation key (`tableLayoutKey`). */
  storageKey: string;
  view: TableView;
  /** Load a saved filter into the bar and apply it. */
  onApply: (saved: SavedFilter) => void;
};

/** The filter bar's saved filters: pick one to apply, save or forget (FR-GRID-12). */
export function SavedFilters({
  storageKey,
  view,
  onApply,
}: Props): React.JSX.Element {
  const { saved, save, remove } = useSavedFilters(storageKey);
  const [chosen, setChosen] = useState('');
  const [naming, setNaming] = useState<string | null>(null);
  if (naming !== null)
    return (
      <SaveName
        name={naming}
        onName={setNaming}
        onSave={(name) => {
          save({ name, rows: view.rows, combinator: view.combinator });
          setChosen(name);
          setNaming(null);
        }}
        onCancel={() => {
          setNaming(null);
        }}
      />
    );
  return (
    <span className={styles.saved}>
      {saved.length > 0 && (
        <SelectField
          aria-label="Saved filters"
          controlSize="compact"
          value={chosen}
          options={[
            { value: '', label: 'Saved filters…' },
            ...saved.map((item) => ({ value: item.name, label: item.name })),
          ]}
          onValueChange={(name) => {
            setChosen(name);
            const found = saved.find((item) => item.name === name);
            if (found) onApply(found);
          }}
        />
      )}
      {chosen && saved.some((item) => item.name === chosen) && (
        <IconButton
          size="sm"
          variant="ghost"
          aria-label={`Forget saved filter ${chosen}`}
          onClick={() => {
            remove(chosen);
            setChosen('');
          }}
        >
          −
        </IconButton>
      )}
      <Button
        size="sm"
        disabled={!view.rows.length}
        onClick={() => {
          setNaming(chosen);
        }}
      >
        Save…
      </Button>
    </span>
  );
}

/** [name] [Save] [Cancel]: Return saves, Escape cancels. */
function SaveName({
  name,
  onName,
  onSave,
  onCancel,
}: {
  name: string;
  onName: (name: string) => void;
  onSave: (name: string) => void;
  onCancel: () => void;
}): React.JSX.Element {
  const trimmed = name.trim();
  return (
    <span className={styles.saved}>
      <TextField
        aria-label="Saved filter name"
        controlSize="compact"
        placeholder="Filter name"
        autoFocus
        value={name}
        onChange={(event) => {
          onName(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return;
          if (event.key === 'Escape') onCancel();
          if (event.key === 'Enter' && trimmed) {
            event.preventDefault();
            onSave(trimmed);
          }
        }}
      />
      <Button
        size="sm"
        variant="primary"
        disabled={!trimmed}
        onClick={() => {
          onSave(trimmed);
        }}
      >
        Save
      </Button>
      <Button size="sm" variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
    </span>
  );
}
