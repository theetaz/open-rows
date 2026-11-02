// SPDX-License-Identifier: Apache-2.0
import type { AccountMode, CodexStatus } from '@openrows/ipc';
import { REASONS } from './aiReasons';
const MODES: Record<AccountMode, string> = {
  chatgpt: 'ChatGPT account',
  apiKey: 'API key in Codex',
  other: 'Codex account',
};
const release = (version: string, newer: boolean): string =>
  `Codex ${version}${newer ? ' (newer than tested)' : ''}`;
/** One explicit line per discovery state; installed never implies signed in. */
export function codexStatusText(status: CodexStatus): string {
  switch (status.state) {
    case 'notInstalled':
      return 'Not installed: install the Codex CLI or set its path';
    case 'signedOut':
      return `Signed out · ${release(status.version, status.newer)}`;
    case 'signedIn':
      return `Signed in · ${MODES[status.mode]}${status.plan ? ` · ${status.plan} plan` : ''} · ${release(status.version, status.newer)}`;
    case 'unsupported':
      return `Unsupported version ${status.version ?? 'unknown'} (tested with ${status.tested})`;
    case 'error':
      return `Error: ${REASONS[status.reason.kind]}`;
  }
}
/** Where the bridge looks for the executable. */
export const codexLocation = (path: string): string =>
  `Codex CLI · ${path || 'found on PATH or in its usual install folders'}`;
