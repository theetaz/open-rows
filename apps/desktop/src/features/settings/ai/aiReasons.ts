// SPDX-License-Identifier: Apache-2.0
import type { AiError } from '@openrows/ipc';
import { t } from '../messages/lazyAi';
/** Fixed user text for each scrubbed provider failure. */
export const REASONS: Record<AiError['kind'], string> = {
  audit: t('auditStorageFailed'),
  auth: t('authenticationFailed'),
  rateLimit: t('rateLimitReached'),
  quota: t('quotaExhausted'),
  network: t('networkFailed'),
  unsupported: t('unsupported'),
  invalidResponse: t('invalidResponse'),
  invalidConfig: t('invalidEndpoint'),
  disabled: t('aiFeaturesOff2'),
  cancelled: t('cancelled'),
  timeout: t('timedOut'),
};
