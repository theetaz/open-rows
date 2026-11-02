// SPDX-License-Identifier: Apache-2.0
import type { AiError } from '@openrows/ipc';
/** Fixed user text for each scrubbed provider failure. */
export const REASONS: Record<AiError['kind'], string> = {
  audit: 'Audit storage failed',
  auth: 'Authentication failed',
  rateLimit: 'Rate limit reached',
  quota: 'Quota exhausted',
  network: 'Network failed',
  unsupported: 'Unsupported',
  invalidResponse: 'Invalid response',
  invalidConfig: 'Invalid endpoint',
  disabled: 'AI features are off',
  cancelled: 'Cancelled',
  timeout: 'Timed out',
};
