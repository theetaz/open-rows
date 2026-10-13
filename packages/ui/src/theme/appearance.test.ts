// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import type { Appearance } from './appearance';
import {
  appearanceKey,
  defaultAppearance,
  fontFamily,
  parseAppearance,
  readAppearance,
  saveAppearance,
} from './appearance';
import { builtInThemes, importTheme, validTheme } from './palettes';
import { appearanceCss, resolvePalette } from './resolveAppearance';
import { paletteRows } from '../tokens/contrast';
it('validates persisted fonts, theme ids, ranges and malformed storage', () => {
  expect(
    parseAppearance({
      uiFont: 'url(evil)',
      monoFont: '__proto__',
      uiSize: Infinity,
      motion: 900,
      lightTheme: 'missing',
      contrast: 0,
      customThemes: [{}],
    }),
  ).toEqual({ ...defaultAppearance, motion: 400, contrast: 80 });
  localStorage.setItem(appearanceKey, '{bad');
  expect(readAppearance()).toEqual(defaultAppearance);
  saveAppearance({ ...defaultAppearance, monoSize: 17, darkTheme: 'iris' });
  expect(readAppearance()).toMatchObject({ monoSize: 17, darkTheme: 'iris' });
  const set = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new DOMException('Denied');
  });
  expect(() => {
    saveAppearance(defaultAppearance);
  }).not.toThrow();
  set.mockRestore();
});
it('imports an independent copy and rejects CSS injection or unreadable canvases', () => {
  const original = { ...builtInThemes[0], id: 'custom-example' };
  expect(validTheme(original)).toBe(true);
  const imported = importTheme(JSON.stringify(original));
  expect(imported.id).not.toBe(original.id);
  expect(imported.light).toEqual(original.light);
  expect(() => importTheme('{}')).toThrow('OpenRows theme');
  expect(() =>
    importTheme(
      JSON.stringify({
        ...original,
        dark: { canvas: '#FFFFFF', accent: '#000000' },
      }),
    ),
  ).toThrow();
  expect(() =>
    importTheme(
      JSON.stringify({
        ...original,
        light: { canvas: 'red;}body{display:none}', accent: '#FFFFFF' },
      }),
    ),
  ).toThrow();
  expect(() => importTheme('x'.repeat(20001))).toThrow('too large');
});
it('generates separate font scales and accessibility overrides', () => {
  const css = appearanceCss({
    ...defaultAppearance,
    uiSize: 16,
    monoSize: 12,
    uiFont: 'arial',
    motion: 200,
  });
  expect(css).toContain('--or-text-mono:12px');
  expect(css).toContain('--or-motion-panel:200ms');
  expect(css).toContain('prefers-reduced-transparency:reduce');
  expect(css).toContain('prefers-contrast:more');
  expect(fontFamily('mono', 'unknown')).toContain('monospace');
});
it('keeps text, badges, controls and focus readable in every preset at control extremes', () => {
  const failures: string[] = [];
  for (const theme of builtInThemes)
    for (const mode of ['light', 'dark'] as const) {
      for (const contrast of [80, 100, 150]) {
        const settings = {
          ...defaultAppearance,
          lightTheme: theme.id,
          darkTheme: theme.id,
          contrast,
        };
        failures.push(...paletteFailures(settings, mode, theme.id));
      }
    }
  expect(
    failures,
    failures
      .filter((item) => !item.includes('contrast=80'))
      .slice(0, 20)
      .join('\n'),
  ).toHaveLength(0);
});

function paletteFailures(
  settings: Appearance,
  mode: 'light' | 'dark',
  id: string,
): string[] {
  return [50, 95, 100].flatMap((opacity) =>
    paletteRows(`${id}/${mode}`, resolvePalette({ ...settings, opacity }, mode))
      .filter((row) => row.ratio < row.minimum)
      .map(
        (row) =>
          `${row.theme}/${row.pair} contrast=${String(settings.contrast)} opacity=${String(opacity)} ratio=${row.ratio.toFixed(2)}`,
      ),
  );
}

it('keeps custom palette extremes and alternate change colors readable', () => {
  const accents = ['#000000', '#ed0000', '#008000', '#0000ff'];
  for (const accent of accents) {
    const theme = {
      id: 'custom-boundary',
      name: 'Boundary',
      light: { canvas: '#e5e5e5', accent },
      dark: { canvas: '#252525', accent },
    };
    expect(validTheme(theme)).toBe(true);
    const settings: Appearance = {
      ...defaultAppearance,
      lightTheme: theme.id,
      darkTheme: theme.id,
      customThemes: [theme],
      contrast: 80,
      changeColors: 'blue-orange',
    };
    const failures = [
      ...paletteFailures(settings, 'light', theme.id),
      ...paletteFailures(settings, 'dark', theme.id),
    ];
    expect(failures, failures.slice(0, 8).join('\n')).toHaveLength(0);
  }
});

it('round-trips versioned colour tokens and refuses unknown, unsafe or low-contrast values', () => {
  const theme = {
    ...builtInThemes[0],
    version: 1,
    id: 'custom-tokens',
    name: 'Tokens',
    light: {
      canvas: '#ffffff',
      accent: '#000000',
      tokens: { 'syntax-keyword': '#123456' },
    },
  };
  const restored = importTheme(JSON.stringify(theme));
  expect(restored.version).toBe(1);
  expect(restored.light.tokens).toEqual(theme.light.tokens);
  for (const changes of [
    { css: 'x' },
    { version: 2 },
    {
      dark: {
        canvas: '#000000',
        accent: '#000000',
        tokens: { 'syntax-keyword': '#010101' },
      },
    },
    { light: { ...theme.light, tokens: { script: 'x' } } },
    { light: { ...theme.light, tokens: { 'row-inserted': '#888888' } } },
  ])
    expect(() =>
      importTheme(JSON.stringify({ ...theme, ...changes })),
    ).toThrow();
});

it('applies separate editor/grid/sidebar fonts and line height through root tokens', () => {
  const appearance = parseAppearance({
    editorFont: 'menlo',
    gridFont: 'courier',
    sidebarFont: 'verdana',
    editorSize: 18,
    gridSize: 14,
    sidebarSize: 16,
    editorLineHeight: 1.8,
  });
  const css = appearanceCss(appearance);
  expect(css).toContain('--or-font-editor:Menlo');
  expect(css).toContain('--or-font-grid:"Courier New"');
  expect(css).toContain('--or-font-sidebar:Verdana');
  expect(css).toContain('--or-text-editor:18px');
  expect(css).toContain('--or-text-grid:14px');
  expect(css).toContain('--or-editor-line-height:1.8');
});

it('preserves the default sidebar half-pixel size and independent font choices', () => {
  expect(parseAppearance({})).toEqual(defaultAppearance);
  saveAppearance(defaultAppearance);
  expect(readAppearance().sidebarSize).toBe(12.5);
  expect(appearanceCss(readAppearance())).toContain(
    '--or-font-sidebar:ui-monospace',
  );
  expect(appearanceCss(readAppearance())).toContain('--or-text-sidebar:12.5px');
  expect(
    appearanceCss(parseAppearance({ sidebarFont: 'verdana', sidebarSize: 16 })),
  ).toContain('--or-font-sidebar:Verdana');
});
