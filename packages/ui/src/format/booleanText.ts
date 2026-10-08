// SPDX-License-Identifier: Apache-2.0
/** TablePlus shows booleans as TRUE/FALSE; constants keep grid painting allocation-free. */
export function booleanText(text: string): string {
  if (text === 'true' || text === 't') return 'TRUE';
  if (text === 'false' || text === 'f') return 'FALSE';
  return text;
}
