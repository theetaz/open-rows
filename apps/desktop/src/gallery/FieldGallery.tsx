// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Icon, SegmentedControl, Switch, TextField } from '@openrows/ui';
import styles from './Gallery.module.css';

const options = [
  { value: 'items', label: 'Items' },
  { value: 'queries', label: 'Queries' },
  { value: 'history', label: 'History' },
] as const;
/** Controlled fields with labelled default, selected, disabled and error examples. */
export function FieldGallery(): React.JSX.Element {
  return (
    <section
      id="fields"
      aria-labelledby="fields-heading"
      className={styles.section}
    >
      <h2 id="fields-heading">Fields & selection</h2>
      <FieldExamples />
      <SelectionExamples />
      <SwitchExamples />
    </section>
  );
}

function FieldExamples(): React.JSX.Element {
  return (
    <div className={styles.grid}>
      <TextField label="Connection name" placeholder="My database" />
      <TextField
        label="Search objects"
        leadingIcon={<Icon name="search" />}
        placeholder="Search…"
      />
      <TextField
        label="Read-only example"
        disabled
        defaultValue="Disabled connection"
      />
      <TextField
        label="Connection URL"
        defaultValue="invalid-url"
        error="Enter a valid connection URL."
      />
      <TextField
        aria-label="Focused field specimen"
        placeholder="Focus specimen"
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
          aria-label="Browse sections"
          options={options}
          value={selection}
          onValueChange={setSelection}
        />
        <span>Arrows / Home / End</span>
      </div>
      <div className={styles.row}>
        <SegmentedControl
          aria-label="Disabled sections"
          options={options}
          value="items"
          onValueChange={setSelection}
          disabled
        />
        <SegmentedControl
          aria-label="Focused segments"
          className={styles.segmentFocusPreview ?? ''}
          options={options}
          value={selection}
          onValueChange={setSelection}
        />
        <span>
          Tab to focus the selected segment; hover for its inset edge.
        </span>
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
        aria-label="Safe mode"
        checked={checked}
        onCheckedChange={setChecked}
      />
      <span>Safe mode {checked ? 'on' : 'off'}</span>
      <Switch
        aria-label="Disabled switch on"
        checked
        disabled
        onCheckedChange={setChecked}
      />
      <Switch
        aria-label="Disabled switch off"
        checked={false}
        disabled
        onCheckedChange={setChecked}
      />
      <Switch
        aria-label="Focused switch specimen"
        checked={focused}
        onCheckedChange={setFocused}
        className={styles.focusPreview}
      />
    </div>
  );
}
