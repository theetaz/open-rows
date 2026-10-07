// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState, useMemo } from 'react';
/** Canvas colors need the same contrast override as CSS-rendered controls. */
export function useAppearanceMedia(): { contrast: boolean } {
  const [contrast, setContrast] = useState(
    () => window.matchMedia('(prefers-contrast: more)').matches,
  );
  useEffect(() => {
    const media = window.matchMedia('(prefers-contrast: more)');
    const listener = (event: MediaQueryListEvent): void => {
      setContrast(event.matches);
    };
    media.addEventListener('change', listener);
    return () => {
      media.removeEventListener('change', listener);
    };
  }, []);
  return useMemo(() => ({ contrast }), [contrast]);
}
