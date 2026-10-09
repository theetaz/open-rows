// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { crashRecoveryStatus } from '@openrows/ipc';

/** Whether this launch follows a run that did not exit cleanly, until dismissed. */
export function useCrashRecovery(): { crashed: boolean; dismiss: () => void } {
  const [crashed, setCrashed] = useState(false);
  useEffect(() => {
    let alive = true;
    // Without a status there is nothing to recover from, so no prompt is shown.
    crashRecoveryStatus().then(
      (value) => {
        if (alive && value) setCrashed(true);
      },
      () => undefined,
    );
    return () => {
      alive = false;
    };
  }, []);
  return {
    crashed,
    dismiss: () => {
      setCrashed(false);
    },
  };
}
