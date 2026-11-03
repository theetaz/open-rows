// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { onTransfersReady, takeTransfers } from '@openrows/ipc';
import type { Connections } from '../connections';
import type { QueryTabs } from '../query';
import { localFileScope } from '../workspace/useNativeFiles';
import { parseTransfer } from './transfer';
import type { Transfer } from './transfer';
import { t } from './messages';

type Input = { tabs: QueryTabs; connections: Connections };

/**
 * Add moved tabs to this window and show their connection and database.
 * The user moved working context here, so its connection is selected (and
 * reconnected) as when they pick it in the sidebar; nothing runs.
 */
function receiveTransfer(transfer: Transfer, input: Input): void {
  input.tabs.receive?.(transfer.tabs);
  for (const utility of transfer.utilities)
    input.tabs.open(
      utility,
      utility.kind === 'monitor'
        ? { monitor: true, title: t('monitor') }
        : { processes: true, title: t('processes') },
    );
  const { scope } = transfer;
  if (scope && scope.connectionId !== localFileScope.connectionId)
    input.connections.select(scope.connectionId, scope.database);
}

/**
 * Take tabs other windows moved here, once this window's profiles are known
 * (so their connection can be selected); later moves arrive by event.
 */
export function useWindowTransfers(input: Input & { enabled: boolean }): void {
  const latest = useRef(input);
  useEffect(() => {
    latest.current = input;
  });
  const ready = input.enabled && input.connections.profilesLoaded === true;
  useEffect(() => {
    if (!ready) return;
    let alive = true;
    let stop: (() => void) | undefined;
    const drain = async (): Promise<void> => {
      const payloads = await takeTransfers();
      for (const payload of payloads) {
        const transfer = parseTransfer(payload);
        if (alive && transfer) receiveTransfer(transfer, latest.current);
      }
    };
    onTransfersReady(() => {
      void drain();
    }).then(
      (unlisten) => {
        if (!alive) {
          unlisten();
          return;
        }
        stop = unlisten;
        void drain();
      },
      () => undefined,
    );
    return () => {
      alive = false;
      stop?.();
    };
  }, [ready]);
}
