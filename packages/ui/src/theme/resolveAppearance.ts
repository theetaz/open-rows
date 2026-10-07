// SPDX-License-Identifier: Apache-2.0
import { light, dark, highContrast } from '../tokens/colours';
import { typography } from '../tokens/typography';
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
  if (settings.changeColors === 'blue-orange') {
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
  const panel = mix(
    half.canvas,
    lightMode ? '#FFFFFF' : '#F0F5FC',
    lightMode ? 0.38 : 0.045,
  );
  return {
    ...base,
    page: half.canvas,
    window: mix(
      half.canvas,
      lightMode ? '#FFFFFF' : '#F0F5FC',
      lightMode ? 0.78 : 0.025,
    ),
    'panel-solid': panel,
    'control-active': mix(
      panel,
      lightMode ? '#FFFFFF' : '#F0F5FC',
      lightMode ? 0.8 : 0.05,
    ),
    accent: half.accent,
    'on-accent': '#FFFFFF',
    focus: lightMode
      ? mix(half.accent, '#000000', 0.15)
      : mix(half.accent, '#FFFFFF', 0.7),
    'danger-fg': lightMode
      ? base['danger-fg']
      : mix(base['danger-fg'], '#FFFFFF', 0.14),
    'row-stripe': alpha(half.accent, lightMode ? 0.07 : 0.12),
    'info-fg': lightMode
      ? mix(half.accent, '#000000', 0.25)
      : mix(half.accent, '#FFFFFF', 0.85),
  };
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
  const contrast = declarations({
    ...highContrast[mode],
    panel: palette['panel-solid'],
  });
  return `${selector}{${overrides}}@media(prefers-color-scheme:${mode}){${system}{${overrides}}}
    @media(prefers-contrast:more){${selector}{${contrast}}@media(prefers-color-scheme:${mode}){${system}{${contrast}}}}
    @media(prefers-reduced-transparency:reduce){${selector}{--or-panel:${palette['panel-solid']};}@media(prefers-color-scheme:${mode}){${system}{--or-panel:${palette['panel-solid']};}}}`;
}
export function appearanceCss(settings: Appearance): string {
  const sizes = Object.entries(typography)
    .filter(([key]) => key.startsWith('text-') && key !== 'text-mono')
    .map(
      ([key, size]) =>
        `--or-${key}:${String((parseFloat(size) * settings.uiSize) / 13)}rem;`,
    )
    .join('');
  return `:root{--or-font-ui:${fontFamily('ui', settings.uiFont)};--or-font-mono:${fontFamily('mono', settings.monoFont)};
    --or-text-mono:${String(settings.monoSize)}px;--or-motion-panel:${String(settings.motion)}ms;${sizes}}
    ${modeStyles(settings, 'light')}${modeStyles(settings, 'dark')}
    :root[data-transparency="reduced"][data-theme]{--or-panel:var(--or-panel-solid);}`;
}
