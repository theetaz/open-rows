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

/** Match UI tokens without duplicating palette values. rem font tokens use the 16px reference. */
export function gridTheme(
  mode: 'light' | 'dark',
  overrides: Partial<
    Pick<GridTheme, 'colours' | 'fontFamily' | 'fontSize'>
  > = {},
  density: GridDensity = 'normal',
): GridTheme {
  const spacing = DENSITY[density];
  return {
    colours: mode === 'light' ? light : dark,
    rowHeight: Math.max(
      parseFloat(geometry['grid-row-height']) + spacing.delta,
      (overrides.fontSize ?? 13) + spacing.padding,
    ),
    fontSize: parseFloat(typography['text-13']) * 16,
    fontFamily: typography['font-mono'],
    ...overrides,
  };
}
