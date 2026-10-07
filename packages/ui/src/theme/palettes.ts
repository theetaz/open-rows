// SPDX-License-Identifier: Apache-2.0
export type ThemeHalf = { canvas: string; accent: string };
export type ThemeDefinition = {
  id: string;
  name: string;
  light: ThemeHalf;
  dark: ThemeHalf;
};
/** Original OpenRows palettes; each theme has independent day/night variants. */
export const builtInThemes: readonly ThemeDefinition[] = [
  {
    id: 'openrows',
    name: 'OpenRows',
    light: { canvas: '#E8EDF5', accent: '#0B6BD6' },
    dark: { canvas: '#151D2A', accent: '#136FDB' },
  },
  {
    id: 'graphite',
    name: 'Graphite',
    light: { canvas: '#EFEFF1', accent: '#5254B8' },
    dark: { canvas: '#1B1B20', accent: '#6359CF' },
  },
  {
    id: 'ocean',
    name: 'Ocean',
    light: { canvas: '#E6F3F9', accent: '#08719D' },
    dark: { canvas: '#10232E', accent: '#087EA4' },
  },
  {
    id: 'forest',
    name: 'Forest',
    light: { canvas: '#EAF3EC', accent: '#207246' },
    dark: { canvas: '#15261E', accent: '#208353' },
  },
  {
    id: 'ember',
    name: 'Ember',
    light: { canvas: '#FAEEE5', accent: '#B74E1E' },
    dark: { canvas: '#2C1D19', accent: '#BE5629' },
  },
  {
    id: 'iris',
    name: 'Iris',
    light: { canvas: '#F2EBFB', accent: '#7842C1' },
    dark: { canvas: '#221B30', accent: '#8750CD' },
  },
];
export function rgb(hex: string): number[] {
  return [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16));
}
function luminance(hex: string): number {
  const channels = rgb(hex).map((value) => {
    const s = value / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return (
    (channels[0] ?? 0) * 0.2126 +
    (channels[1] ?? 0) * 0.7152 +
    (channels[2] ?? 0) * 0.0722
  );
}
export function mix(a: string, b: string, weight: number): string {
  const other = rgb(b);
  return (
    '#' +
    rgb(a)
      .map((value, index) =>
        Math.round(value * (1 - weight) + (other[index] ?? 0) * weight)
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  );
}
function validHalf(value: unknown, mode: 'light' | 'dark'): value is ThemeHalf {
  if (!value || typeof value !== 'object') return false;
  const half = value as Record<string, unknown>;
  if (typeof half.canvas !== 'string' || typeof half.accent !== 'string')
    return false;
  if (
    ![half.canvas, half.accent].every((color) => /^#[\da-f]{6}$/i.test(color))
  )
    return false;
  if (luminance(half.accent) > 0.1833) return false;
  const lightness = luminance(half.canvas);
  return mode === 'light' ? lightness >= 0.75 : lightness <= 0.02;
}
/** Constrain canvas brightness so generated text remains readable. */
export function validTheme(value: unknown): value is ThemeDefinition {
  if (!value || typeof value !== 'object') return false;
  const t = value as Record<string, unknown>;
  return (
    typeof t.id === 'string' &&
    /^custom-[\w-]{1,80}$/.test(t.id) &&
    typeof t.name === 'string' &&
    t.name.trim().length > 0 &&
    t.name.length <= 50 &&
    validHalf(t.light, 'light') &&
    validHalf(t.dark, 'dark')
  );
}
export function importTheme(text: string): ThemeDefinition {
  if (text.length > 20000) throw new Error('Theme JSON is too large.');
  const value: unknown = JSON.parse(text);
  if (!validTheme(value))
    throw new Error(
      'Use an OpenRows theme with a name, light and dark palettes, and six-digit hex colors. Light backgrounds must be light; dark backgrounds must be dark. Accents need readable white labels.',
    );
  return { ...value, id: `custom-${crypto.randomUUID()}` };
}
