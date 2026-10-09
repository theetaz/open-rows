// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useRef } from 'react';
import { useTheme } from '@openrows/ui';
import { gridTheme } from '@openrows/grid';
import type { GridHandle, GridTheme } from '@openrows/grid';
import { generalSettings } from '../features/settings/general';
/** Change paint settings in place, retaining selection, scroll and tile caches. */
export function useGridAppearance(): {
  theme: React.RefObject<GridTheme>;
  handle: React.RefObject<GridHandle | undefined>;
} {
  const { resolved, gridAppearance } = useTheme();
  const { gridDensity } = generalSettings.use();
  const theme = useRef(gridTheme(resolved, gridAppearance, gridDensity));
  const handle = useRef<GridHandle | undefined>(undefined);
  useLayoutEffect(() => {
    theme.current = gridTheme(resolved, gridAppearance, gridDensity);
    handle.current?.setTheme?.(theme.current);
  }, [resolved, gridAppearance, gridDensity]);
  return { theme, handle };
}
