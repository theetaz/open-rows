// SPDX-License-Identifier: Apache-2.0
import { createReadStream } from 'node:fs';

/** Refuse a normal/release executable before it can open any application data. */
export async function assertAutomationBinary(path) {
  const remaining = new Set([
    'automation_reply',
    'automation_observations',
    'OPENROWS_E2E_ROOT',
    'bridge.sock',
  ]);
  let boundary = '';
  for await (const bytes of createReadStream(path)) {
    const chunk = boundary + bytes.toString('latin1');
    for (const marker of remaining)
      if (chunk.includes(marker)) remaining.delete(marker);
    boundary = chunk.slice(-32);
  }
  if (remaining.size)
    throw new Error(
      'E2E requires a debug executable built with test-automation',
    );
}
