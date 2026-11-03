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
import { t } from './messages/lazyPanes';

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
    title: t('columnHeaders'),
    description: t('whetherCopiedRows'),
    label: t('copyColumnHeaders'),
    options: [
      { value: 'format', label: t('csvOnly') },
      { value: 'always', label: t('always') },
      { value: 'never', label: t('never') },
    ],
  },
  {
    key: 'nulls',
    title: t('nullValues'),
    description: t('howSqlNull'),
    label: t('copyNullValues'),
    options: [
      { value: 'format', label: t('nullPlainCopies') },
      { value: 'text', label: t('alwaysNull') },
      { value: 'empty', label: t('alwaysEmpty') },
    ],
  },
  {
    key: 'quote',
    title: t('quotes'),
    description: t('quoteValuesContain'),
    label: t('copyQuoteStyle'),
    options: [
      { value: 'minimal', label: t('onlyWhenNeeded') },
      { value: 'all', label: t('everyValue') },
    ],
  },
  {
    key: 'quoteChar',
    title: t('quoteCharacter'),
    description: t('characterWrapsQuoted'),
    label: t('copyQuoteCharacter'),
    options: [
      { value: 'double', label: t('doubleQuote') },
      { value: 'single', label: t('singleQuote') },
    ],
  },
  {
    key: 'delimiter',
    title: t('csvDelimiter'),
    description: t('separatorBetweenCsv'),
    label: t('csvDelimiter'),
    options: [
      { value: 'comma', label: t('comma') },
      { value: 'semicolon', label: t('semicolon') },
      { value: 'tab', label: t('tab') },
      { value: 'pipe', label: t('pipe') },
    ],
  },
  {
    key: 'newline',
    title: t('lineEndings'),
    description: t('whatEndsEach'),
    label: t('copyLineEndings'),
    options: [
      { value: 'lf', label: t('lfMacosLinux') },
      { value: 'crlf', label: t('crlfWindows') },
    ],
  },
  {
    key: 'decimal',
    title: t('decimalSeparator'),
    description: t('separatorWrittenNumeric'),
    label: t('copyDecimalSeparator'),
    options: [
      { value: 'point', label: t('point15') },
      { value: 'comma', label: t('comma15') },
    ],
  },
];

/** Settings ▸ Tables ▸ Copy: plain (TSV), CSV and HTML copy defaults (FR-GRID-13). */
export function CopySection(): React.JSX.Element {
  const settings = copySettings.useValue();
  const [conflict, setConflict] = useState<string | null>(null);
  return (
    <section aria-label={t('copy')}>
      <h3>{t('copy')}</h3>
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
