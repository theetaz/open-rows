// SPDX-License-Identifier: Apache-2.0
import { startSpike, finishSpike } from '@openrows/ipc';
import { SpikeSession, withTimeout } from './lifecycle';
import { measurePageTimer } from './timer';

/** Query spike mode and await native window reveal before loading any worker code. */
export async function startIpcSpike(): Promise<void> {
  let session: SpikeSession | undefined;
  try {
    if (!(await withTimeout(startSpike(), 10_000, 'Spike startup handshake')))
      return;
    session = new SpikeSession();
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
