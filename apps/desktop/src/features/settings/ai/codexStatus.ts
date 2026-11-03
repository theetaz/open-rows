// SPDX-License-Identifier: Apache-2.0
import type { AccountMode, CodexStatus } from '@openrows/ipc';
import { REASONS } from './aiReasons';
import { t } from '../messages/lazyAi';
const MODES: Record<AccountMode, string> = {
  chatgpt: t('chatgptAccount'),
  apiKey: t('apiKeyCodex'),
  other: t('codexAccount'),
};
const release = (version: string, newer: boolean): string =>
  newer ? t('codexVersionNewer', { version }) : t('codexVersion', { version });
/** One explicit line per discovery state; installed never implies signed in. */
export function codexStatusText(status: CodexStatus): string {
  switch (status.state) {
    case 'notInstalled':
      return t('notInstalledInstall');
    case 'signedOut':
      return t('signedOutVersion', {
        version: release(status.version, status.newer),
      });
    case 'signedIn':
      return status.plan
        ? t('signedModesPlan', {
            mode: MODES[status.mode],
            plan: status.plan,
            version: release(status.version, status.newer),
          })
        : t('signedModesVersion', {
            mode: MODES[status.mode],
            version: release(status.version, status.newer),
          });
    case 'unsupported':
      return t('unsupportedVersionVersion', {
        version: status.version ?? t('unknownValue'),
        tested: status.tested,
      });
    case 'error':
      return t('errorReasons', { reason: REASONS[status.reason.kind] });
  }
}
/** Where the bridge looks for the executable. */
export const codexLocation = (path: string): string =>
  t('codexCliPath', { path: path || t('foundPathUsual') });
