// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { material } from '../../tokens/material';
import { useReducedMotion } from '../useReducedMotion';
/** Retain a closing panel only for its configured exit animation. */
export function usePanelPresence(open: boolean): boolean {
  const [previous, setPrevious] = useState(open);
  const [closing, setClosing] = useState(false);
  const reduced = useReducedMotion();
  if (previous !== open) {
    setPrevious(open);
    setClosing(!open);
  }
  useEffect(() => {
    if (!closing) return;
    const timer = window.setTimeout(
      () => {
        setClosing(false);
      },
      reduced ? 0 : parseFloat(material['motion-panel']),
    );
    return () => {
      window.clearTimeout(timer);
    };
  }, [closing, reduced]);
  return open || closing;
}
