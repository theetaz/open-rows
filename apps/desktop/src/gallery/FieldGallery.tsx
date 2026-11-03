// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Icon, SegmentedControl, Switch, TextField } from '@openrows/ui';
import styles from './Gallery.module.css';
import { t } from './messages';

const options = [
  { value: 'items', label: t('items') },
  { value: 'queries', label: t('queries') },
  { value: 'history', label: t('history') },
] as const;
/** Controlled fields with labelled default, selected, disabled and error examples. */
export function FieldGallery(): React.JSX.Element {
  return (
    <section
      id="fields"
      aria-labelledby="fields-heading"
      className={styles.section}
    >
      <h2 id="fields-heading">{t('fieldsSelection')}</h2>
      <FieldExamples />
      <SelectionExamples />
      <SwitchExamples />
    </section>
  );
}

function FieldExamples(): React.JSX.Element {
  return (
    <div className={styles.grid}>
      <TextField label={t('connectionName')} placeholder={t('myDatabase')} />
      <TextField
        label={t('searchObjects')}
        leadingIcon={<Icon name="search" />}
        placeholder={t('search')}
      />
      <TextField
        label={t('readOnlyExample')}
        disabled
        defaultValue="Disabled connection"
      />
      <TextField
        label={t('connectionUrl')}
        defaultValue="invalid-url"
        error={t('enterValidConnection')}
      />
      <TextField
        aria-label={t('focusedFieldSpecimen')}
        placeholder={t('focusSpecimen')}
        className={styles.fieldFocusPreview}
      />
    </div>
  );
}

function SelectionExamples(): React.JSX.Element {
  const [selection, setSelection] = useState<'items' | 'queries' | 'history'>(
    'items',
  );
  return (
    <>
      <div className={styles.row}>
        <SegmentedControl
          aria-label={t('browseSections')}
          options={options}
          value={selection}
          onValueChange={setSelection}
        />
        <span>{t('arrowsHomeEnd')}</span>
      </div>
      <div className={styles.row}>
        <SegmentedControl
          aria-label={t('disabledSections')}
          options={options}
          value="items"
          onValueChange={setSelection}
          disabled
        />
        <SegmentedControl
          aria-label={t('focusedSegments')}
          className={styles.segmentFocusPreview ?? ''}
          options={options}
          value={selection}
          onValueChange={setSelection}
        />
        <span>{t('tabFocusSelected')}</span>
      </div>
    </>
  );
}

function SwitchExamples(): React.JSX.Element {
  const [checked, setChecked] = useState(true);
  const [focused, setFocused] = useState(false);
  return (
    <div className={styles.row}>
      <Switch
        aria-label={t('safeMode')}
        checked={checked}
        onCheckedChange={setChecked}
      />
      <span>{checked ? t('safeMode2') : t('safeModeOff')}</span>
      <Switch
        aria-label={t('disabledSwitch')}
        checked
        disabled
        onCheckedChange={setChecked}
      />
      <Switch
        aria-label={t('disabledSwitchOff')}
        checked={false}
        disabled
        onCheckedChange={setChecked}
      />
      <Switch
        aria-label={t('focusedSwitchSpecimen')}
        checked={focused}
        onCheckedChange={setFocused}
        className={styles.focusPreview}
      />
    </div>
  );
}
