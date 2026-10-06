// SPDX-License-Identifier: Apache-2.0
import { transferBuffers } from './transfer';
import type { WorkerReply } from './types';

import { decodeTile } from './decode';
import { convertDisplayTile } from '@openrows/grid/display-tile';
import type { WindowRequest } from '@openrows/ipc';
const worker = self as unknown as {
  onmessage: (
    event: MessageEvent<{
      id: number;
      bytes: ArrayBuffer;
      display?: WindowRequest;
    }>,
  ) => void;
  postMessage: (reply: WorkerReply, transfers?: Transferable[]) => void;
};
worker.onmessage = ({ data }) => {
  try {
    const result = decodeTile(data.bytes);
    if (data.display) {
      worker.postMessage({
        kind: 'display',
        id: data.id,
        result: convertDisplayTile(result.tile, data.display),
      });
      return;
    }
    worker.postMessage(
      { kind: 'decoded', id: data.id, result },
      transferBuffers(result),
    );
  } catch (error) {
    worker.postMessage({ kind: 'error', id: data.id, message: String(error) });
  }
};
worker.postMessage({ kind: 'ready' });
