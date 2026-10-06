// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { getAppInfo } from '@openrows/ipc';
import type { AppInfo } from '@openrows/ipc';

type InfoState =
  | { status: 'loading' }
  | { status: 'success'; info: AppInfo }
  | { status: 'error'; message: string };

/** Read metadata once per mounted card, ignoring late replies after unmount. */
export function useAppInfo(): InfoState {
  const [state, setState] = useState<InfoState>({ status: 'loading' });
  useEffect(() => {
    let active = true;
    void getAppInfo().then(
      (info) => {
        if (active) setState({ status: 'success', info });
      },
      (error: unknown) => {
        if (active)
          setState({
            status: 'error',
            message: error instanceof Error ? error.message : String(error),
          });
      },
    );
    return () => {
      active = false;
    };
  }, []);
  return state;
}
