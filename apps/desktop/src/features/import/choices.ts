// SPDX-License-Identifier: Apache-2.0
import type {
  BadRowsDto,
  ConflictDto,
  ImportFormatDto,
  TransactionDto,
  TypeCategoryDto,
} from '@openrows/ipc';
import { t } from './messages';

type Choice<Value extends string> = { value: Value; label: string };

export const formatChoices: readonly Choice<ImportFormatDto>[] = [
  { value: 'delimited', label: t('csvTsv') },
  { value: 'jsonArray', label: t('jsonArray') },
  { value: 'jsonLines', label: t('ndjson') },
];

/** Delimiters by the character the core expects. */
export const delimiterChoices: readonly Choice<string>[] = [
  { value: ',', label: t('comma') },
  { value: ';', label: t('semicolon') },
  { value: '\t', label: t('tab') },
  { value: '|', label: t('pipe') },
];

/** Quote characters; `none` reads quotes as text. */
export const quoteChoices: readonly Choice<string>[] = [
  { value: '"', label: t('doubleQuote') },
  { value: "'", label: t('singleQuote') },
  { value: 'none', label: t('none') },
];

/** Common encodings by WHATWG label. */
export const encodingChoices: readonly Choice<string>[] = [
  { value: 'utf-8', label: t('utf8') },
  { value: 'utf-16le', label: t('utf16Le') },
  { value: 'utf-16be', label: t('utf16') },
  { value: 'windows-1252', label: t('westernWindows1252') },
  { value: 'iso-8859-2', label: t('centralEuropeanIso') },
  { value: 'shift_jis', label: t('japaneseShiftJis') },
  { value: 'gbk', label: t('chineseGbk') },
];

export const conflictChoices: readonly Choice<ConflictDto>[] = [
  { value: 'error', label: t('stopError') },
  { value: 'skip', label: t('skipRow') },
  { value: 'update', label: t('updateExistingRow') },
];

export const transactionChoices: readonly Choice<TransactionDto>[] = [
  { value: 'single', label: t('oneTransactionAll') },
  { value: 'batches', label: t('commitEachBatch') },
];

export const badRowChoices: readonly Choice<BadRowsDto>[] = [
  { value: 'skip', label: t('skipReportThem') },
  { value: 'abort', label: t('stopImport') },
];

const categoryLabels: Readonly<Record<TypeCategoryDto, string>> = {
  integer: 'integer',
  decimal: 'number',
  boolean: 'boolean',
  date: 'date',
  timestamp: 'timestamp',
  timestampTz: t('timestampZone'),
  json: 'JSON',
  text: 'text',
};

/** The inferred category as a short label. */
export const categoryLabel = (category: TypeCategoryDto): string =>
  categoryLabels[category];
