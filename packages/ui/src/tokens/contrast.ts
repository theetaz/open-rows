// SPDX-License-Identifier: Apache-2.0
import { light, dark, highContrast } from './colours.ts';

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
function ratio(foreground: string, background: Colour): number {
  const first = luminance(composite(parse(foreground), background));
  const second = luminance(background);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
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
function paletteRows(theme: string, palette: typeof light): Row[] {
  const backgrounds = surfaces(palette);
  const rows: Row[] = [];
  for (const [name, background] of Object.entries(backgrounds)) {
    for (const foreground of [
      'text',
      'secondary',
      'tertiary',
      'focus',
      'danger-fg',
    ] as const) {
      rows.push({
        theme,
        pair: `${foreground}/${name}`,
        ratio: ratio(palette[foreground], background),
        minimum: foreground === 'focus' ? 3 : 4.5,
      });
    }
    rows.push(...badgeRows({ theme, palette, name, background }));
  }
  for (const background of ['accent', 'destructive'] as const)
    rows.push({
      theme,
      pair: `on-accent/${background}`,
      ratio: ratio(palette['on-accent'], parse(palette[background])),
      minimum: 4.5,
    });
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
