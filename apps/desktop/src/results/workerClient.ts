// SPDX-License-Identifier: Apache-2.0
import type { DecodedTile, WorkerReply } from './types';
import type { DisplayTile } from '@openrows/grid/display-tile';
import type { WindowRequest } from '@openrows/ipc';
import InlineArrowWorker from './arrow.worker?worker&inline';

interface Pending {
  resolve: (tile: ReceivedTile | DisplayTile) => void;
  reject: (error: Error) => void;
  postedAt: number;
}

/** Page-clock elapsed time includes input transfer, decoding and the typed reply. */
interface ReceivedTile extends DecodedTile {
  transferMs: number;
}

/** Own one lazy Arrow worker; transfer payloads and reject all work on shutdown. */
export class ArrowWorker {
  private readonly worker: Worker;
  private readonly pending = new Map<number, Pending>();
  private readonly ready: Promise<void>;
  private rejectReady: ((error: Error) => void) | undefined;
  private sequence = 0;
  private failure: Error | undefined;

  constructor() {
    this.worker = new InlineArrowWorker();
    this.ready = new Promise((resolve, reject) => {
      this.rejectReady = reject;
      this.worker.onmessage = ({ data }: MessageEvent<WorkerReply>) => {
        if (data.kind === 'ready') resolve();
        else this.receive(data);
      };
      this.worker.onerror = (event) => {
        const error = new Error(event.message || 'Arrow worker failed');
        reject(error);
        this.stop(error);
      };
      this.worker.onmessageerror = () => {
        const error = new Error('Invalid Arrow worker message');
        reject(error);
        this.stop(error);
      };
    });
  }

  /** Warm worker loading before measuring steady-state tile transport. */
  async initialize(): Promise<void> {
    await this.ready;
  }

  /** Decode with ownership transferred to the worker. */
  async decode(bytes: ArrayBuffer): Promise<ReceivedTile> {
    const result = await this.request(bytes);
    if (!('tile' in result)) throw new Error('Unexpected display reply');
    return result;
  }
  /** Format grid cells in the worker; S1's binary decode protocol stays unchanged. */
  async display(
    bytes: ArrayBuffer,
    display: WindowRequest,
  ): Promise<DisplayTile> {
    const result = await this.request(bytes, display);
    if (!('cells' in result)) throw new Error('Unexpected binary reply');
    return result;
  }
  private async request(
    bytes: ArrayBuffer,
    display?: WindowRequest,
  ): Promise<ReceivedTile | DisplayTile> {
    await this.ready;
    if (this.failure) throw this.failure;
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      const pending = { resolve, reject, postedAt: 0 };
      this.pending.set(id, pending);
      try {
        pending.postedAt = performance.now();
        this.worker.postMessage(
          display ? { id, bytes, display } : { id, bytes },
          [bytes],
        );
      } catch (error) {
        this.pending.delete(id);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  /** Stop the worker and settle all outstanding requests. */
  stop(error = new Error('Arrow worker stopped')): void {
    this.failure = error;
    this.rejectReady?.(error);
    this.worker.terminate();
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }

  private receive(reply: Exclude<WorkerReply, { kind: 'ready' }>): void {
    const receivedAt = performance.now();
    const pending = this.pending.get(reply.id);
    if (!pending) return;
    this.pending.delete(reply.id);
    if (reply.kind === 'decoded')
      pending.resolve({
        ...reply.result,
        transferMs: receivedAt - pending.postedAt,
      });
    else if (reply.kind === 'display') pending.resolve(reply.result);
    else pending.reject(new Error(reply.message));
  }
}
