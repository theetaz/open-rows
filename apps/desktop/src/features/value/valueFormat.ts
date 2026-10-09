// SPDX-License-Identifier: Apache-2.0
/** Values up to this size load with the inspector; larger ones wait for "Load full value". */
export const AUTO_LOAD_BYTES = 1024 * 1024;
/** Bytes shown in the hex view; a dump of a huge value stays a few thousand lines. */
export const HEX_SHOWN_BYTES = 64 * 1024;
/** Characters of text rendered at once; the rest of a huge value stays unrendered. */
export const TEXT_SHOWN_CHARS = 200_000;

const UNITS = ['KB', 'MB', 'GB', 'TB'] as const;

/** A byte count for people: `812 B`, `1.4 MB`. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${String(bytes)} B`;
  let size = bytes / 1024;
  let unit = 0;
  while (size >= 1024 && unit < UNITS.length - 1) {
    size /= 1024;
    unit += 1;
  }
  const digits = size < 10 ? 1 : 0;
  return `${size.toFixed(digits)} ${UNITS[unit] ?? 'TB'}`;
}

/** Text cut to `limit` characters, with whether anything was cut. */
export function clipText(
  text: string,
  limit: number,
): { text: string; clipped: boolean } {
  return text.length > limit
    ? { text: text.slice(0, limit), clipped: true }
    : { text, clipped: false };
}
