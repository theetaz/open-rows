// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';

export const sidebarRange = { min: 200, max: 360, initial: 228 };
export const inspectorRange = { min: 280, max: 440, initial: 316 };
export function isCompact(width: number): boolean {
  return width < 1100;
}
/** Window geometry changes, not pointer motion, drive responsive React state. */
export function useCompact(): boolean {
  const [compact, setCompact] = useState(() => isCompact(window.innerWidth));
  useEffect(() => {
    const resize = (): void => {
      setCompact(isCompact(window.innerWidth));
    };
    window.addEventListener('resize', resize);
    return () => {
      window.removeEventListener('resize', resize);
    };
  }, []);
  return compact;
}
/** An explicit overlay request is cleared automatically when crossing the threshold. */
export function useInspectorOpen(compact: boolean): {
  open: boolean;
  toggle: () => void;
  show: () => void;
} {
  const [choice, setChoice] = useState({ compact, open: !compact });
  if (choice.compact !== compact) setChoice({ compact, open: !compact });
  return {
    open: choice.compact === compact ? choice.open : !compact,
    show: () => {
      setChoice({ compact, open: true });
    },
    toggle: () => {
      setChoice({ compact, open: !choice.open });
    },
  };
}
