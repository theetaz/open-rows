// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';

/** Run `action` whenever `trigger` changes to a non-null value. */
export function useTrigger(trigger: string | null, action: () => void): void {
  const latest = useRef(action);
  useEffect(() => {
    latest.current = action;
  });
  useEffect(() => {
    if (trigger !== null) latest.current();
  }, [trigger]);
}
