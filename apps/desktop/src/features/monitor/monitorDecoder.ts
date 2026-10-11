// SPDX-License-Identifier: Apache-2.0
import InlineMonitorWorker from './monitor.worker?worker&inline';
import type { MonitorTable } from './monitorArrow';

type Pending = {
  id: number;
  resolve: (table: MonitorTable) => void;
  reject: (error: Error) => void;
};
type Reply = { id: number; table?: MonitorTable; error?: string };

/** Own one lazy decoder with one transferred page in flight; stop settles pending work. */
export class MonitorDecoder {
  private readonly worker = new InlineMonitorWorker();
  private pending: Pending | null = null;
  private sequence = 0;
  private failure: Error | null = null;

  constructor() {
    this.worker.onmessage = ({ data }: MessageEvent<Reply>) => {
      const pending = this.pending;
      if (!pending || pending.id !== data.id) return;
      this.pending = null;
      if (data.table) pending.resolve(data.table);
      else
        pending.reject(
          new Error(data.error ?? 'Invalid monitor decoder reply'),
        );
    };
    this.worker.onerror = () => {
      this.stop(new Error('Monitor decoder failed'));
    };
    this.worker.onmessageerror = () => {
      this.stop(new Error('Invalid monitor decoder message'));
    };
  }

  /** Transfer the bounded page and retain its null validity and source metadata. */
  decode(bytes: ArrayBuffer): Promise<MonitorTable> {
    if (this.failure) return Promise.reject(this.failure);
    if (this.pending)
      return Promise.reject(new Error('Monitor decoder is busy'));
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      this.pending = { id, resolve, reject };
      try {
        this.worker.postMessage({ id, bytes }, [bytes]);
      } catch {
        this.pending = null;
        reject(new Error('Monitor page transfer failed'));
      }
    });
  }

  /** Release the worker on close, scope change, pause, lock or decode failure. */
  stop(error = new Error('Monitor decoder stopped')): void {
    this.failure = error;
    this.worker.terminate();
    this.pending?.reject(error);
    this.pending = null;
  }
}
