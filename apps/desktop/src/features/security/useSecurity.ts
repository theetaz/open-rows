// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { onSecurityChanged, securityStatus } from '@openrows/ipc';
import type { SecurityStatus } from '@openrows/ipc';
/** Read lock status before showing data; event subscription precedes the initial read. */
export function useSecurity(): SecurityStatus {
  const [status, setStatus] = useState<SecurityStatus>({
    locked: true,
    available: false,
    notice: null,
  });
  useEffect(() => {
    let live = true;
    let sequence = 0;
    let stop: (() => void) | undefined;
    const refresh = (): void => {
      const request = ++sequence;
      securityStatus()
        .then((value) => {
          if (live && request === sequence) setStatus(value);
        })
        .catch(() => undefined);
    };
    onSecurityChanged((locked) => {
      if (locked) setStatus((previous) => ({ ...previous, locked: true }));
      refresh();
    })
      .then((unlisten) => {
        if (live) {
          stop = unlisten;
          refresh();
        } else unlisten();
      })
      .catch(() => undefined);
    return () => {
      live = false;
      stop?.();
    };
  }, []);
  return status;
}
