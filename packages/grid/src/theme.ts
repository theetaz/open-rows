// SPDX-License-Identifier: Apache-2.0
import { light, dark, geometry, typography } from '@openrows/ui/tokens';
import type { GridTheme } from './model';

/** Grid row spacing, from the Settings ▸ General choice. */
export type GridDensity = 'compact' | 'normal' | 'comfortable';

/** Row height change from the token height, and the minimum padding around text. */
const DENSITY: Readonly<
  Record<GridDensity, { delta: number; padding: number }>
> = {
  compact: { delta: -6, padding: 6 },
  normal: { delta: 0, padding: 12 },
  comfortable: { delta: 6, padding: 18 },
};

/** The header is one pixel taller than a row (24 / 25 px at normal density). */
const HEADER_EXTRA =
  parseFloat(geometry['grid-header-height']) -
  parseFloat(geometry['grid-row-height']);

/** Header title and type caption sizes (v4 grid: 11.5 / 10.5 px). */
const HEADER_SIZE = parseFloat(typography['text-11-5']) * 16;
const CAPTION_SIZE = HEADER_SIZE - 1;

/**
 * Match UI tokens without duplicating palette values: TablePlus density, 12 px
 * monospace cells, 11.5 px semibold UI-font titles with a 10.5 px type caption. rem tokens use the 16px reference.
 */
export function gridTheme(
  mode: 'light' | 'dark',
  overrides: Partial<
    Pick<GridTheme, 'colours' | 'fontFamily' | 'fontSize'>
  > = {},
  density: GridDensity = 'normal',
): GridTheme {
  const spacing = DENSITY[density];
  const fontSize = parseFloat(typography['text-grid']) * 16;
  const rowHeight = Math.max(
    parseFloat(geometry['grid-row-height']) + spacing.delta,
    (overrides.fontSize ?? fontSize) + spacing.padding,
  );
  return {
    colours: mode === 'light' ? light : dark,
    rowHeight,
    headerHeight: rowHeight + HEADER_EXTRA,
    headerFont: `${typography['weight-semibold']} ${String(HEADER_SIZE)}px ${typography['font-ui']}`,
    captionFont: `${typography['weight-regular']} ${String(CAPTION_SIZE)}px ${typography['font-ui']}`,
    fontSize,
    fontFamily: typography['font-grid'],
    ...overrides,
  };
}
