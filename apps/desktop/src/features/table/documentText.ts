// SPDX-License-Identifier: Apache-2.0
import type { CellValue } from './cellValue';
import { t } from './messages/lazyData';

/**
 * Why `text` cannot be staged as a MongoDB document: it must parse as one
 * JSON/EJSON object. BSON types and ranges are checked natively before review.
 */
export function documentError(text: string): string | null {
  try {
    const value: unknown = JSON.parse(text);
    return value !== null && typeof value === 'object' && !Array.isArray(value)
      ? null
      : t('useJsonEjson');
  } catch {
    return t('enterValidJson');
  }
}

/** Whether a staged `__document` value would be refused (staging stays off). */
export function invalidDocument(value: CellValue | undefined): boolean {
  return value?.kind === 'text' && documentError(value.value) !== null;
}
