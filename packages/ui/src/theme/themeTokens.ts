// SPDX-License-Identifier: Apache-2.0
import { luminance } from './palettes';
import { light, dark } from '../tokens/colours';

/** The complete, code-free allowlist of per-palette JSON token overrides. */
export const themeTokenKeys = [
  'text',
  'secondary',
  'syntax-keyword',
  'syntax-string',
  'syntax-number',
  'syntax-comment',
  'row-inserted',
  'row-deleted',
  'cell-edited',
] as const;
/** A safe foreground or edit-state colour token. */
export type ThemeToken = (typeof themeTokenKeys)[number];
/** Optional overrides; missing foregrounds retain their validated defaults. */
export type ThemeTokens = Partial<Record<ThemeToken, string>>;

/** Contrast compares relative sRGB luminance according to WCAG AA. */
function ratio(first: string, second: string): number {
  const a = luminance(first);
  const b = luminance(second);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** Reject unknown keys and refuse default/custom foregrounds failing 4.5:1 on custom fills. */
export function validTokens(
  value: unknown,
  canvas: string,
  mode: 'light' | 'dark',
): value is ThemeTokens {
  if (value === undefined) return true;
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const entries = Object.entries(value);
  if (
    entries.some(
      ([key, color]) =>
        !themeTokenKeys.includes(key as ThemeToken) ||
        typeof color !== 'string' ||
        !/^#[\da-f]{6}$/i.test(color),
    )
  )
    return false;
  const tokens = value as ThemeTokens;
  const fills = [
    canvas,
    ...['row-inserted', 'row-deleted', 'cell-edited'].flatMap(
      (key) => tokens[key as ThemeToken] ?? [],
    ),
  ];
  const defaults = mode === 'light' ? light : dark;
  const text = themeTokenKeys
    .filter(
      (key) => !['row-inserted', 'row-deleted', 'cell-edited'].includes(key),
    )
    .map((key) => tokens[key] ?? defaults[key]);
  return text.every((color) =>
    fills.every((fill) => ratio(color, fill) >= 4.5),
  );
}
