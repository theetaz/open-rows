// SPDX-License-Identifier: Apache-2.0
import { finishSpike, spikeProgress } from '@openrows/ipc';
import type { TileTiming, GridRun, EditorRun } from '@openrows/ipc';
import { measurePageTimer } from './timer';

/** Bound a spike operation and clear the watchdog on every settlement. */
export async function withTimeout<T>(
  operation: Promise<T>,
  milliseconds: number,
  stage: string,
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`${stage} timed out after ${String(milliseconds)} ms`));
    }, milliseconds);
  });
  try {
    return await Promise.race([operation, deadline]);
  } finally {
    clearTimeout(timer);
  }
}

/** Browsers report deferred ResizeObserver delivery as an error; it is not a failure. */
const BENIGN_ERROR = /^ResizeObserver loop/;

/** Own progress, global failures and exactly one terminal report for the spike. */
export class SpikeSession {
  readonly samples: TileTiming[] = [];
  readonly grid: GridRun[] = [];
  readonly editor: EditorRun[] = [];
  readonly timer = measurePageTimer();
  private terminal: Promise<void> | undefined;
  private progressQueue = Promise.resolve();
  private readonly abort = new AbortController();
  private readonly error = (event: ErrorEvent): void => {
    if (BENIGN_ERROR.test(event.message)) {
      this.progress('warning', event.message);
      return;
    }
    void this.finish(event.message || 'Window script/resource error');
  };
  private readonly rejection = (event: PromiseRejectionEvent): void => {
    void this.finish(String(event.reason));
  };

  /** Install handlers before loading the spike or creating its worker. */
  constructor() {
    window.addEventListener('error', this.error, { capture: true });
    window.addEventListener('unhandledrejection', this.rejection);
  }

  /** Stop sampling when a global error terminates the session. */
  get signal(): AbortSignal {
    return this.abort.signal;
  }

  /** Queue milestones without awaiting diagnostic I/O inside timed samples. */
  progress(stage: string, detail = ''): void {
    this.progressQueue = this.progressQueue.then(() =>
      withTimeout(spikeProgress(stage, detail), 2_000, `Progress ${stage}`),
    );
    // Attach immediately so a rejected diagnostic command cannot recurse through the global handler.
    void this.progressQueue.catch((error: unknown) =>
      this.finish(String(error)),
    );
  }

  /** Persist an error or successful samples once, even when failures race. */
  finish(error: string | null = null): Promise<void> {
    if (this.terminal) return this.terminal;
    this.abort.abort();
    window.removeEventListener('error', this.error, { capture: true });
    window.removeEventListener('unhandledrejection', this.rejection);
    this.terminal = this.report(error);
    return this.terminal;
  }

  private async report(error: string | null): Promise<void> {
    try {
      await this.progressQueue;
      if (error)
        await withTimeout(
          spikeProgress('error', error),
          2_000,
          'Error progress',
        );
      await withTimeout(
        spikeProgress('finishing', `${String(this.samples.length)} samples`),
        2_000,
        'Finishing progress',
      );
    } catch (caught) {
      error = error ?? String(caught);
    }
    await finishSpike({
      samples: this.samples,
      timer: this.timer,
      error,
      ...(this.grid.length ? { grid: this.grid } : {}),
      ...(this.editor.length ? { editor: this.editor } : {}),
    });
  }
}
