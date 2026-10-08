// SPDX-License-Identifier: Apache-2.0
import { typography } from '../tokens/typography';
import { builtInThemes, validTheme } from './palettes';
import type { ThemeDefinition } from './palettes';
export type Appearance = {
  lightTheme: string;
  darkTheme: string;
  contrast: number;
  opacity: number;
  motion: number;
  changeColors: 'red-green' | 'blue-orange';
  uiFont: string;
  monoFont: string;
  uiSize: number;
  monoSize: number;
  customThemes: ThemeDefinition[];
};
export const uiFonts = {
  system: typography['font-ui'],
  helvetica: '"Helvetica Neue", Helvetica, Arial, sans-serif',
  arial: 'Arial, Helvetica, sans-serif',
  verdana: 'Verdana, Geneva, sans-serif',
};
export const monoFonts = {
  system: typography['font-mono'],
  menlo: 'Menlo, Consolas, monospace',
  consolas: 'Consolas, Menlo, monospace',
  courier: '"Courier New", Courier, monospace',
};
export const defaultAppearance: Appearance = {
  lightTheme: 'openrows',
  darkTheme: 'openrows',
  contrast: 100,
  opacity: 95,
  motion: 0,
  changeColors: 'red-green',
  uiFont: 'system',
  monoFont: 'system',
  uiSize: 13,
  monoSize: 12,
  customThemes: [],
};
export const appearanceKey = 'openrows.appearance.v1';
function bounded(
  value: unknown,
  min: number,
  max: number,
  fallback: number,
): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.min(max, Math.max(min, Math.round(value)))
    : fallback;
}
/** All persisted fields are validated; stored strings never become raw CSS. */
export function parseAppearance(value: unknown): Appearance {
  if (!value || typeof value !== 'object') return { ...defaultAppearance };
  const v = value as Record<string, unknown>;
  const customThemes = Array.isArray(v.customThemes)
    ? v.customThemes.filter(validTheme).slice(0, 24)
    : [];
  const ids = new Set([...builtInThemes, ...customThemes].map((t) => t.id));
  return {
    lightTheme:
      typeof v.lightTheme === 'string' && ids.has(v.lightTheme)
        ? v.lightTheme
        : 'openrows',
    darkTheme:
      typeof v.darkTheme === 'string' && ids.has(v.darkTheme)
        ? v.darkTheme
        : 'openrows',
    contrast: bounded(v.contrast, 80, 150, 100),
    opacity: bounded(v.opacity, 50, 100, 95),
    motion: bounded(v.motion, 0, 400, 0),
    changeColors:
      v.changeColors === 'blue-orange' ? 'blue-orange' : 'red-green',
    uiFont:
      typeof v.uiFont === 'string' && Object.hasOwn(uiFonts, v.uiFont)
        ? v.uiFont
        : 'system',
    monoFont:
      typeof v.monoFont === 'string' && Object.hasOwn(monoFonts, v.monoFont)
        ? v.monoFont
        : 'system',
    uiSize: bounded(v.uiSize, 11, 18, 13),
    monoSize: bounded(v.monoSize, 11, 18, defaultAppearance.monoSize),
    customThemes,
  };
}
export function readAppearance(): Appearance {
  try {
    return parseAppearance(
      JSON.parse(localStorage.getItem(appearanceKey) ?? 'null'),
    );
  } catch {
    return { ...defaultAppearance };
  }
}
export function saveAppearance(value: Appearance): void {
  try {
    localStorage.setItem(appearanceKey, JSON.stringify(value));
  } catch {
    /* Preferences remain usable in a restricted webview. */
  }
}
export function fontFamily(kind: 'ui' | 'mono', id: string): string {
  const fonts: Record<string, string> = kind === 'ui' ? uiFonts : monoFonts;
  return fonts[id] ?? typography[kind === 'ui' ? 'font-ui' : 'font-mono'];
}
