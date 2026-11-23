// SPDX-License-Identifier: Apache-2.0
import type { EnvDto } from '@openrows/ipc';

/**
 * The colour marking an environment in the window chrome: the theme's tag
 * foreground, which the contrast report covers, or a custom tag's own colour.
 */
export function environmentColour(env: EnvDto): string {
  if (env.kind === 'custom') return env.color;
  return `var(--or-${env.kind}-fg)`;
}
