// SPDX-License-Identifier: Apache-2.0
import { MIN_COLUMN_WIDTH } from '@openrows/grid';
import type { ColumnLayout } from './columnLayout';

/**
 * Store one column's width. Lives apart from `columnLayout` because the shell
 * reads layouts at startup and the grid package must stay out of that bundle.
 */
export function withWidth(
  layout: ColumnLayout,
  name: string,
  width: number,
): ColumnLayout {
  return {
    ...layout,
    widths: {
      ...layout.widths,
      [name]: Math.max(MIN_COLUMN_WIDTH, Math.round(width)),
    },
  };
}
