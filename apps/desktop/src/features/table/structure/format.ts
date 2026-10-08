// SPDX-License-Identifier: Apache-2.0
const units = ['KB', 'MB', 'GB', 'TB'] as const;

/** `81 KB`, `1 MB`: binary units rounded to whole numbers, like TablePlus. */
export function formatBytes(bytes: number | null): string {
  if (bytes === null) return '—';
  if (bytes < 1024) return `${String(bytes)} bytes`;
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${String(Math.round(value))} ${units[unit] ?? 'TB'}`;
}

/** `1,788` for a planner estimate, or an em dash when unknown. */
export function formatRows(rows: number | null): string {
  return rows === null ? '—' : rows.toLocaleString('en-US');
}
