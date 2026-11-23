// SPDX-License-Identifier: Apache-2.0
import { light, dark, highContrast } from '../tokens/colours';
import { typography } from '../tokens/typography';
import { contrastRatio } from '../tokens/contrast';
import { SOLID } from '../tokens/solid';
import { fontFamily } from './appearance';
import type { Appearance } from './appearance';
import { builtInThemes, mix, rgb } from './palettes';
type Palette = typeof light;
function alpha(hex: string, opacity: number): string {
  return `rgba(${rgb(hex).join(',')},${String(opacity)})`;
}
/** Generate coherent surfaces, preserve semantic status colors and text roles. */
export function resolvePalette(
  settings: Appearance,
  mode: 'light' | 'dark',
): Palette {
  const base = mode === 'light' ? light : dark;
  const id = mode === 'light' ? settings.lightTheme : settings.darkTheme;
  const theme =
    [...builtInThemes, ...settings.customThemes].find(
      (item) => item.id === id,
    ) ?? builtInThemes[0];
  const half = theme?.[mode];
  let palette = { ...base };
  if (half && id !== 'openrows') palette = tintedPalette(base, half, mode);
  palette.panel = alpha(palette['panel-solid'], settings.opacity / 100);
  const contrast = (settings.contrast - 100) / 100;
  if (contrast !== 0) {
    palette.secondary = mix(
      base.secondary,
      contrast > 0 ? base.text : palette.page,
      Math.abs(contrast) * (contrast < 0 ? 0.15 : 1),
    );
    palette.tertiary = palette.secondary;
    palette.separator = alpha(
      base.text,
      Math.max(0.03, (mode === 'light' ? 0.055 : 0.12) + contrast * 0.3),
    );
    palette['control-stroke'] = alpha(
      base.text,
      Math.max(0.08, (mode === 'light' ? 0.12 : 0.18) + contrast * 0.35),
    );
  }
  for (const key of READABLE)
    palette[key] = readable(palette[key], labelSurfaces(palette), mode);
  palette.tertiary = readable(palette.tertiary, [[palette.page]], mode, 3);
  for (const tone of TONES)
    palette[`${tone}-fg`] = readable(
      palette[`${tone}-fg`],
      badgeSurfaces(palette, tone),
      mode,
    );
  palette = changePalette(palette, settings.changeColors, mode);
  return { ...palette, ...half?.tokens };
}
function changePalette(
  palette: Palette,
  colors: Appearance['changeColors'],
  mode: 'light' | 'dark',
): Palette {
  if (colors === 'blue-orange') {
    palette['row-inserted'] = alpha(
      mode === 'light' ? '#08719D' : '#64B9FF',
      0.14,
    );
    palette['row-deleted'] = alpha(
      mode === 'light' ? '#B74E1E' : '#FFAB64',
      0.14,
    );
  }

  return palette;
}
function tintedPalette(
  base: Palette,
  half: { canvas: string; accent: string },
  mode: 'light' | 'dark',
): Palette {
  const lightMode = mode === 'light';
  return {
    ...base,
    page: half.canvas,
    'panel-solid': half.canvas,
    window: lightMode
      ? mix(half.canvas, '#FFFFFF', 0.85)
      : mix(half.canvas, '#000000', 0.3),
    'control-active': lightMode ? '#FFFFFF' : mix(half.canvas, '#FFFFFF', 0.1),
    accent: lightMode ? half.accent : mix(half.accent, '#FFFFFF', 0.45),
    'accent-fill': half.accent,
    'accent-soft': alpha(half.accent, lightMode ? 0.13 : 0.2),
    'on-accent': '#FFFFFF',
    // Tinted canvases are lighter or darker; keep NULL/EMPTY markers at 3:1.
    placeholder: mix(base.placeholder, lightMode ? '#000000' : '#FFFFFF', 0.12),
    focus: lightMode
      ? mix(half.accent, '#000000', 0.15)
      : mix(half.accent, '#FFFFFF', 0.7),
    'field-focus': alpha(half.accent, 0.5),
    'danger-fg': lightMode
      ? base['danger-fg']
      : mix(base['danger-fg'], '#FFFFFF', 0.14),
    'row-stripe': alpha(half.accent, lightMode ? 0.05 : 0.08),
    toolbar: lightMode
      ? mix(half.canvas, '#FFFFFF', 0.85)
      : mix(half.canvas, '#000000', 0.3),
    group: half.canvas,
    'info-fg': lightMode
      ? mix(half.accent, '#000000', 0.25)
      : mix(half.accent, '#FFFFFF', 0.85),
  };
}
/** Text roles kept at 4.5:1 whatever canvas, accent or contrast is chosen. */
const READABLE = [
  'secondary',
  'label-tertiary',
  'danger-fg',
  'accent',
] as const;
/** A translucent `rgba()` (or hex) laid over an opaque hex base, as hex. */
function flatten(colour: string, base: string): string {
  if (colour.startsWith('#')) return colour;
  const [r = 0, g = 0, b = 0, a = 1] = (colour.match(/[\d.]+/g) ?? []).map(
    Number,
  );
  const hex =
    '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('');
  return mix(base, hex, a);
}
/** The floating sidebar's clear glass over the tinted window, as hex. */
function sidebarSurface(palette: Palette): string {
  const tinted = flatten(palette['window-tint'], palette.page);
  return flatten(palette['glass-sidebar'], tinted);
}
/** Surfaces a label sits on: the canvas, groups, floating glass and fills. */
function labelSurfaces(palette: Palette): [string, string?][] {
  const bases = [
    palette.page,
    palette['panel-solid'],
    palette.group,
    sidebarSurface(palette),
    flatten(palette['glass-float'], palette.page),
    flatten(palette['glass-sheet'], palette.page),
  ];
  return [
    ...bases.flatMap((base): [string, string?][] => [
      [base],
      [palette['control-fill'], base],
    ]),
    [palette['accent-soft'], palette.window],
    [palette['row-stripe'], palette.window],
  ];
}
/** Status pills: the tone's soft fill on each solid surface. */
const TONES = [
  'warning',
  'success',
  'info',
  'neutral',
  'danger',
  'local',
  'dev',
  'staging',
  'prod',
] as const;
function badgeSurfaces(
  palette: Palette,
  tone: (typeof TONES)[number],
): [string, string?][] {
  const fill = palette[`${tone}-bg`];
  return [
    palette.page,
    palette.window,
    palette['panel-solid'],
    palette.group,
    sidebarSurface(palette),
    flatten(palette['glass-float'], palette.page),
    flatten(palette['glass-sheet'], palette.page),
  ].map((base) => [fill, base]);
}
/** Darken (light) or lighten (dark) a label only as far as AA requires. */
function readable(
  colour: string,
  surfaces: readonly [string, string?][],
  mode: 'light' | 'dark',
  minimum = 4.5,
): string {
  let channels = rgb(colour);
  let label = colour;
  for (let step = 0; step < 255; step += 1) {
    if (
      surfaces.every(
        ([fill, under]) => contrastRatio(label, fill, under) >= minimum + 0.05,
      )
    )
      break;
    channels = channels.map((channel) =>
      Math.max(0, Math.min(255, channel + (mode === 'light' ? -1 : 1))),
    );
    label =
      '#' +
      channels.map((channel) => channel.toString(16).padStart(2, '0')).join('');
  }
  return label;
}
function declarations(palette: Record<string, string>): string {
  return Object.entries(palette)
    .map(([key, value]) => `--or-${key}:${value};`)
    .join('');
}
function modeStyles(settings: Appearance, mode: 'light' | 'dark'): string {
  const palette = resolvePalette(settings, mode);
  const selector = `:root[data-theme="${mode}"]`;
  const system =
    mode === 'light'
      ? ':root:not([data-theme="dark"]):not([data-theme="light"])'
      : ':root[data-theme="system"]';
  const overrides = `color-scheme:${mode};` + declarations(palette);
  const solid = declarations({
    ...SOLID,
    panel: palette['panel-solid'],
    toolbar: palette.page,
  });
  const contrast = declarations({
    ...highContrast[mode],
    panel: palette['panel-solid'],
  });
  return `${selector}{${overrides}}@media(prefers-color-scheme:${mode}){${system}{${overrides}}}
    @media(prefers-contrast:more){${selector}{${contrast}}@media(prefers-color-scheme:${mode}){${system}{${contrast}}}}
    @media(prefers-reduced-transparency:reduce){${selector}{${solid}}@media(prefers-color-scheme:${mode}){${system}{${solid}}}}`;
}
export function appearanceCss(settings: Appearance): string {
  const sizes = Object.entries(typography)
    .filter(
      ([key]) =>
        key.startsWith('text-') &&
        !['text-mono', 'text-editor', 'text-grid', 'text-sidebar'].includes(
          key,
        ),
    )
    .map(
      ([key, size]) =>
        `--or-${key}:${String((parseFloat(size) * settings.uiSize) / 13)}rem;`,
    )
    .join('');
  return `:root{--or-font-ui:${fontFamily('ui', settings.uiFont)};--or-font-mono:${fontFamily('mono', settings.monoFont)};
    --or-font-editor:${fontFamily('mono', settings.editorFont)};--or-font-grid:${fontFamily('mono', settings.gridFont)};--or-font-sidebar:${fontFamily('sidebar', settings.sidebarFont)};
    --or-text-editor:${String(settings.editorSize)}px;--or-text-grid:${String(settings.gridSize)}px;--or-text-sidebar:${String(settings.sidebarSize)}px;--or-editor-line-height:${String(settings.editorLineHeight)};
    --or-text-mono:${String(settings.monoSize)}px;--or-motion-panel:${String(settings.motion)}ms;${sizes}}
    ${modeStyles(settings, 'light')}${modeStyles(settings, 'dark')}
    :root[data-transparency="reduced"][data-theme]{${declarations(SOLID)}}`;
}
