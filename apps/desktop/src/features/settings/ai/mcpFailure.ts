// SPDX-License-Identifier: Apache-2.0
import { t } from '../messages/lazyAi';
/** What went wrong with a user MCP server, and what to do about it. */
export function mcpFailure(error: unknown, server: string): string {
  const kind =
    error && typeof error === 'object' && 'kind' in error
      ? String(error.kind)
      : '';
  switch (kind) {
    case 'timeout':
      return t('serverDidNot', { server });
    case 'network':
      return t('couldNotStart', { server });
    case 'protocol':
      return t('serverReturnedMore', { server });
    case 'invalid':
      return t('theseSettingsNot');
    case 'secret':
    case 'auth':
      return t('osKeychainRefused');
    case 'unsupported':
      return t('turnAiFeatures');
    case 'cancelled':
      return t('cancelledBecauseMcp');
    case 'locked':
      return t('openrowsLockedUnlock');
    case 'storage':
      return t('couldNotWrite');
    default:
      return t('serverMcpOperation', { server });
  }
}
