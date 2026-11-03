// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, IconButton, SelectField, TextField } from '@openrows/ui';
import { useSavedFilters } from '../presentationStore';
import type { SavedFilter } from '../presentation';
import type { TableView } from '../model';
import styles from '../TableExtras.module.css';
import { t } from '../messages/lazyData';

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
          save({
            name,
            rows: view.rows,
            groups: view.groups,
            combinator: view.combinator,
          });
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
          aria-label={t('savedFilters')}
          controlSize="compact"
          value={chosen}
          options={[
            { value: '', label: t('savedFilters2') },
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
          aria-label={t('forgetSavedFilter', { chosen })}
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
        {t('save')}
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
        aria-label={t('savedFilterName')}
        controlSize="compact"
        placeholder={t('filterName2')}
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
        {t('save2')}
      </Button>
      <Button size="sm" variant="ghost" onClick={onCancel}>
        {t('cancel')}
      </Button>
    </span>
  );
}
