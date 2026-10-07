// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useRef } from 'react';
import { useTheme } from '@openrows/ui';
import { gridTheme } from '@openrows/grid';
import type { GridHandle, GridTheme } from '@openrows/grid';
/** Change paint settings in place, retaining selection, scroll and tile caches. */
export function useGridAppearance(): {
  theme: React.RefObject<GridTheme>;
  handle: React.RefObject<GridHandle | undefined>;
} {
  const { resolved, gridAppearance } = useTheme();
  const theme = useRef(gridTheme(resolved, gridAppearance));
  const handle = useRef<GridHandle | undefined>(undefined);
  useLayoutEffect(() => {
    theme.current = gridTheme(resolved, gridAppearance);
    handle.current?.setTheme?.(theme.current);
  }, [resolved, gridAppearance]);
  return { theme, handle };
}
