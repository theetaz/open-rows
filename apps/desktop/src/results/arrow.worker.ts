// SPDX-License-Identifier: Apache-2.0
import { transferBuffers } from './transfer';
import type { WorkerReply } from './types';

import { decodeTile } from './decode';
const worker = self as unknown as {
  onmessage: (event: MessageEvent<{ id: number; bytes: ArrayBuffer }>) => void;
  postMessage: (reply: WorkerReply, transfers?: Transferable[]) => void;
};
worker.onmessage = ({ data }) => {
  try {
    const result = decodeTile(data.bytes);
    worker.postMessage(
      { kind: 'decoded', id: data.id, result },
      transferBuffers(result),
    );
  } catch (error) {
    worker.postMessage({ kind: 'error', id: data.id, message: String(error) });
  }
};
worker.postMessage({ kind: 'ready' });
