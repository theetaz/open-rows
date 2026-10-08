// SPDX-License-Identifier: Apache-2.0
import { light, dark, highContrast } from './colours.ts';
import { geometry } from './geometry.ts';

type Colour = readonly [number, number, number, number];
function parse(value: string): Colour {
  if (value.startsWith('#'))
    return [
      parseInt(value.slice(1, 3), 16),
      parseInt(value.slice(3, 5), 16),
      parseInt(value.slice(5, 7), 16),
      1,
    ];
  const values = value.match(/[\d.]+/g)?.map(Number);
  if (!values || values.length !== 4)
    throw new Error(`Unsupported colour: ${value}`);
  return [values[0] ?? 0, values[1] ?? 0, values[2] ?? 0, values[3] ?? 1];
}
function composite(foreground: Colour, background: Colour): Colour {
  const alpha = foreground[3];
  const channel = (index: 0 | 1 | 2): number =>
    foreground[index] * alpha + background[index] * (1 - alpha);
  return [channel(0), channel(1), channel(2), 1];
}
function luminance(colour: Colour): number {
  const linear = colour.slice(0, 3).map((channel) => {
    const value = channel / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return (
    (linear[0] ?? 0) * 0.2126 +
    (linear[1] ?? 0) * 0.7152 +
    (linear[2] ?? 0) * 0.0722
  );
}
function colourRatio(foreground: Colour, background: Colour): number {
  const first = luminance(foreground);
  const second = luminance(background);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}
function ratio(foreground: string, background: Colour): number {
  return colourRatio(composite(parse(foreground), background), background);
}
/** WCAG ratio for a foreground composited over an opaque sRGB background. */
export function contrastRatio(foreground: string, background: string): number {
  return ratio(foreground, parse(background));
}
type Row = { theme: string; pair: string; ratio: number; minimum: number };
function surfaces(palette: typeof light): Record<string, Colour> {
  const page = parse(palette.page);
  const window = parse(palette.window);
  const panel = composite(parse(palette.panel), page);
  const panelOnWindow = composite(parse(palette.panel), window);
  const bases = {
    page,
    window,
    panel,
    'panel/window': panelOnWindow,
    'panel-solid': parse(palette['panel-solid']),
    ...Object.fromEntries(
      (
        ['row-stripe', 'row-inserted', 'row-deleted', 'cell-edited'] as const
      ).map((state) => [state, composite(parse(palette[state]), window)]),
    ),
  };
  const controls = Object.fromEntries(
    Object.entries(bases).flatMap(([name, base]) => {
      const fill = composite(parse(palette['control-fill']), base);
      return [
        [`fill/${name}`, fill],
        [
          `nested-fill/${name}`,
          composite(parse(palette['control-fill']), fill),
        ],
      ];
    }),
  );
  return { ...bases, ...controls, active: parse(palette['control-active']) };
}
export function paletteRows(theme: string, palette: typeof light): Row[] {
  const backgrounds = surfaces(palette);
  const rows: Row[] = [];
  for (const [name, background] of Object.entries(backgrounds)) {
    for (const foreground of [
      'text',
      'secondary',
      'tertiary',
      'placeholder',
      'focus',
      'danger-fg',
    ] as const) {
      rows.push({
        theme,
        pair: `${foreground}/${name}`,
        ratio: ratio(palette[foreground], background),
        minimum:
          foreground === 'focus' || foreground === 'placeholder' ? 3 : 4.5,
      });
    }
    rows.push(...badgeRows({ theme, palette, name, background }));
  }
  for (const background of [
    'accent',
    'destructive',
    'connection-pg',
    'connection-prod',
    'connection-ch',
    'connection-sqlite',
  ] as const)
    rows.push({
      theme,
      pair: `on-accent/${background}`,
      ratio: ratio(palette['on-accent'], parse(palette[background])),
      minimum: 4.5,
    });
  rows.push(...railRows(theme, palette));
  return rows;
}
/** Full WCAG sRGB coverage of text/focus pairs, composited glass and badge fills. */
export function contrastRows(): Row[] {
  return [
    ...paletteRows('light', light),
    ...paletteRows('dark', dark),
    ...paletteRows('high-contrast-light', { ...light, ...highContrast.light }),
    ...paletteRows('high-contrast-dark', { ...dark, ...highContrast.dark }),
  ];
}

function badgeRows({
  theme,
  palette,
  name,
  background,
}: {
  theme: string;
  palette: typeof light;
  name: string;
  background: Colour;
}): Row[] {
  const rows: Row[] = [];
  for (const tone of [
    'warning',
    'success',
    'info',
    'neutral',
    'danger',
    'local',
    'dev',
    'staging',
    'prod',
  ] as const) {
    rows.push({
      theme,
      pair: `${tone}/${name}`,
      ratio: ratio(
        palette[`${tone}-fg`],
        composite(parse(palette[`${tone}-bg`]), background),
      ),
      minimum: 4.5,
    });
  }
  return rows;
}

function railRows(theme: string, palette: typeof light): Row[] {
  const opacity = Number(geometry['rail-inactive-opacity']);
  const page = parse(palette.page);
  const faded = (value: string): Colour => {
    const colour = parse(value);
    return composite([colour[0], colour[1], colour[2], opacity], page);
  };
  return (['connection-pg', 'connection-ch', 'connection-sqlite'] as const).map(
    (engine) => ({
      theme,
      pair: `rail-inactive/${engine}`,
      ratio: colourRatio(faded(palette['on-accent']), faded(palette[engine])),
      minimum: 4.5,
    }),
  );
}
