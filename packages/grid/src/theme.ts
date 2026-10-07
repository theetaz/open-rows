// SPDX-License-Identifier: Apache-2.0
import { light, dark, geometry, typography } from '@openrows/ui/tokens';
import type { GridTheme } from './model';
/** Match UI tokens without duplicating palette values. rem font tokens use the 16px reference. */
export function gridTheme(
  mode: 'light' | 'dark',
  overrides: Partial<
    Pick<GridTheme, 'colours' | 'fontFamily' | 'fontSize'>
  > = {},
): GridTheme {
  return {
    colours: mode === 'light' ? light : dark,
    rowHeight: Math.max(
      parseFloat(geometry['grid-row-height']),
      (overrides.fontSize ?? 13) + 12,
    ),
    fontSize: parseFloat(typography['text-13']) * 16,
    fontFamily: typography['font-mono'],
    ...overrides,
  };
}
