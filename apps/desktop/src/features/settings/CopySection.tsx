// SPDX-License-Identifier: Apache-2.0
import { SelectField } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import {
  copySettings,
  updateCopySettings,
} from '../../results/copy/copySettings';
import type { CopySettings } from '../../results/copy/copySettings';
import styles from '../appearance/Appearance.module.css';

const HEADERS: readonly { value: CopySettings['headers']; label: string }[] = [
  { value: 'format', label: 'CSV only' },
  { value: 'always', label: 'Always' },
  { value: 'never', label: 'Never' },
];
const NULLS: readonly { value: CopySettings['nulls']; label: string }[] = [
  { value: 'format', label: 'NULL in plain copies, empty in CSV' },
  { value: 'text', label: 'Always NULL' },
  { value: 'empty', label: 'Always empty' },
];
const QUOTES: readonly { value: CopySettings['quote']; label: string }[] = [
  { value: 'minimal', label: 'Only when needed' },
  { value: 'all', label: 'Every value' },
];

/** Settings ▸ General ▸ Copy: plain (TSV) and CSV copy defaults (FR-GRID-13). */
export function CopySection(): React.JSX.Element {
  const settings = copySettings.use();
  return (
    <section aria-label="Copy">
      <h3>Copy</h3>
      <div className={styles.settingsGroup}>
        <SettingRow
          title="Column headers"
          description="Whether copied rows start with the column names. Copy with Headers always includes them."
        >
          <SelectField
            aria-label="Copy column headers"
            value={settings.headers}
            options={HEADERS}
            onValueChange={(headers) => {
              updateCopySettings({ headers });
            }}
          />
        </SettingRow>
        <SettingRow
          title="NULL values"
          description="How SQL NULL is written in plain and CSV copies."
        >
          <SelectField
            aria-label="Copy NULL values as"
            value={settings.nulls}
            options={NULLS}
            onValueChange={(nulls) => {
              updateCopySettings({ nulls });
            }}
          />
        </SettingRow>
        <SettingRow
          title="Quotes"
          description="Quote values that contain a delimiter, quote or line break, or every value. Quotes inside values are doubled."
        >
          <SelectField
            aria-label="Copy quote style"
            value={settings.quote}
            options={QUOTES}
            onValueChange={(quote) => {
              updateCopySettings({ quote });
            }}
          />
        </SettingRow>
      </div>
    </section>
  );
}
