// SPDX-License-Identifier: Apache-2.0
import type {
  BadRowsDto,
  ConflictDto,
  ImportFormatDto,
  TransactionDto,
  TypeCategoryDto,
} from '@openrows/ipc';

type Choice<Value extends string> = { value: Value; label: string };

export const formatChoices: readonly Choice<ImportFormatDto>[] = [
  { value: 'delimited', label: 'CSV / TSV' },
  { value: 'jsonArray', label: 'JSON array' },
  { value: 'jsonLines', label: 'NDJSON' },
];

/** Delimiters by the character the core expects. */
export const delimiterChoices: readonly Choice<string>[] = [
  { value: ',', label: 'Comma ( , )' },
  { value: ';', label: 'Semicolon ( ; )' },
  { value: '\t', label: 'Tab' },
  { value: '|', label: 'Pipe ( | )' },
];

/** Quote characters; `none` reads quotes as text. */
export const quoteChoices: readonly Choice<string>[] = [
  { value: '"', label: 'Double quote ( " )' },
  { value: "'", label: "Single quote ( ' )" },
  { value: 'none', label: 'None' },
];

/** Common encodings by WHATWG label. */
export const encodingChoices: readonly Choice<string>[] = [
  { value: 'utf-8', label: 'UTF-8' },
  { value: 'utf-16le', label: 'UTF-16 LE' },
  { value: 'utf-16be', label: 'UTF-16 BE' },
  { value: 'windows-1252', label: 'Western (Windows-1252)' },
  { value: 'iso-8859-2', label: 'Central European (ISO-8859-2)' },
  { value: 'shift_jis', label: 'Japanese (Shift JIS)' },
  { value: 'gbk', label: 'Chinese (GBK)' },
];

export const conflictChoices: readonly Choice<ConflictDto>[] = [
  { value: 'error', label: 'Stop with an error' },
  { value: 'skip', label: 'Skip the row' },
  { value: 'update', label: 'Update the existing row' },
];

export const transactionChoices: readonly Choice<TransactionDto>[] = [
  { value: 'single', label: 'One transaction (all or nothing)' },
  { value: 'batches', label: 'Commit each batch' },
];

export const badRowChoices: readonly Choice<BadRowsDto>[] = [
  { value: 'skip', label: 'Skip and report them' },
  { value: 'abort', label: 'Stop the import' },
];

const categoryLabels: Readonly<Record<TypeCategoryDto, string>> = {
  integer: 'integer',
  decimal: 'number',
  boolean: 'boolean',
  date: 'date',
  timestamp: 'timestamp',
  timestampTz: 'timestamp with zone',
  json: 'JSON',
  text: 'text',
};

/** The inferred category as a short label. */
export const categoryLabel = (category: TypeCategoryDto): string =>
  categoryLabels[category];
