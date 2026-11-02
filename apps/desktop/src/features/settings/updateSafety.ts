// SPDX-License-Identifier: Apache-2.0
import type { SessionStatus } from '../connections/sessions';

/** Require closed tabs and no connected or opening profiles. */
export function updateIsSafe(
  tabCount: number,
  states: readonly SessionStatus['state'][],
): boolean {
  return (
    tabCount === 0 &&
    states.every((state) => state === 'disconnected' || state === 'failed')
  );
}
