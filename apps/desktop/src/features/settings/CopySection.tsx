// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { SelectField } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import {
  copySettings,
  updateCopySettings,
} from '../../results/copy/copySettings';
import type { CopySettings } from '../../results/copy/copySettings';
import styles from '../appearance/Appearance.module.css';

type CopyRow = {
  [K in keyof CopySettings]: {
    key: K;
    title: string;
    description: string;
    label: string;
    options: readonly { value: CopySettings[K]; label: string }[];
  };
}[keyof CopySettings];

/** Each copy setting with its choices, in pane order. */
const ROWS: readonly CopyRow[] = [
  {
    key: 'headers',
    title: 'Column headers',
    description:
      'Whether copied rows start with the column names. Copy with Headers always includes them.',
    label: 'Copy column headers',
    options: [
      { value: 'format', label: 'CSV only' },
      { value: 'always', label: 'Always' },
      { value: 'never', label: 'Never' },
    ],
  },
  {
    key: 'nulls',
    title: 'NULL values',
    description: 'How SQL NULL is written in plain, CSV and HTML copies.',
    label: 'Copy NULL values as',
    options: [
      { value: 'format', label: 'NULL in plain copies, empty in CSV' },
      { value: 'text', label: 'Always NULL' },
      { value: 'empty', label: 'Always empty' },
    ],
  },
  {
    key: 'quote',
    title: 'Quotes',
    description:
      'Quote values that contain a delimiter, quote or line break, or every value. Quotes inside values are doubled.',
    label: 'Copy quote style',
    options: [
      { value: 'minimal', label: 'Only when needed' },
      { value: 'all', label: 'Every value' },
    ],
  },
  {
    key: 'quoteChar',
    title: 'Quote character',
    description: 'The character that wraps quoted values.',
    label: 'Copy quote character',
    options: [
      { value: 'double', label: 'Double quote "' },
      { value: 'single', label: "Single quote '" },
    ],
  },
  {
    key: 'delimiter',
    title: 'CSV delimiter',
    description: 'Separator between CSV fields; plain copies use tabs.',
    label: 'CSV delimiter',
    options: [
      { value: 'comma', label: 'Comma ,' },
      { value: 'semicolon', label: 'Semicolon ;' },
      { value: 'tab', label: 'Tab' },
      { value: 'pipe', label: 'Pipe |' },
    ],
  },
  {
    key: 'newline',
    title: 'Line endings',
    description: 'What ends each copied row.',
    label: 'Copy line endings',
    options: [
      { value: 'lf', label: 'LF (macOS, Linux)' },
      { value: 'crlf', label: 'CRLF (Windows)' },
    ],
  },
  {
    key: 'decimal',
    title: 'Decimal separator',
    description:
      'Separator written in numeric columns of plain and CSV copies.',
    label: 'Copy decimal separator',
    options: [
      { value: 'point', label: 'Point 1.5' },
      { value: 'comma', label: 'Comma 1,5' },
    ],
  },
];

/** Settings ▸ Tables ▸ Copy: plain (TSV), CSV and HTML copy defaults (FR-GRID-13). */
export function CopySection(): React.JSX.Element {
  const settings = copySettings.useValue();
  const [conflict, setConflict] = useState<string | null>(null);
  return (
    <section aria-label="Copy">
      <h3>Copy</h3>
      <div className={styles.settingsGroup}>
        {ROWS.map((row) => (
          <SettingRow
            key={row.key}
            title={row.title}
            description={row.description}
          >
            <SelectField
              aria-label={row.label}
              value={settings[row.key]}
              options={row.options}
              onValueChange={(value) => {
                setConflict(updateCopySettings({ [row.key]: value }));
              }}
            />
          </SettingRow>
        ))}
        {conflict && <p role="alert">{conflict}</p>}
      </div>
    </section>
  );
}
