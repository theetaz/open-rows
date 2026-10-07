// SPDX-License-Identifier: Apache-2.0
import { startSpike, finishSpike } from '@openrows/ipc';
import { SpikeSession, withTimeout } from './lifecycle';
import { measurePageTimer } from './timer';

/** Query spike mode and await native window reveal before loading any worker code. */
export async function startIpcSpike(): Promise<void> {
  let session: SpikeSession | undefined;
  try {
    const mode = await withTimeout(
      startSpike(),
      10_000,
      'Spike startup handshake',
    );
    if (!mode) return;
    session = new SpikeSession();
    if (mode === 'grid') {
      const { runGridSpike } = await withTimeout(
        import('./grid'),
        10_000,
        'Grid module loading',
      );
      await runGridSpike(session);
      return;
    }
    if (mode === 'editor') {
      const { runEditorSpike } = await withTimeout(
        import('./editor'),
        10_000,
        'Editor module loading',
      );
      await runEditorSpike(session);
      return;
    }
    const { runIpcSpike } = await withTimeout(
      import('./ipc'),
      10_000,
      'Spike module loading',
    );
    await runIpcSpike(session);
  } catch (error) {
    if (session) await session.finish(String(error));
    else
      await finishSpike({
        samples: [],
        timer: measurePageTimer(),
        error: String(error),
      });
  }
}
