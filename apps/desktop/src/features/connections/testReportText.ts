// SPDX-License-Identifier: Apache-2.0
import { t, tn } from './messages/profile';

/** How a median round trip feels: under 50 ms fast, under 200 ms moderate. */
type LatencyQuality = 'fast' | 'moderate' | 'slow';

export function latencyQuality(medianMs: number): LatencyQuality {
  if (medianMs < 50) return 'fast';
  if (medianMs < 200) return 'moderate';
  return 'slow';
}

/** `expires in 12 days`, `expires today`, `expired 3 days ago`. */
export function expiryText(daysLeft: number | null): string | null {
  if (daysLeft === null) return null;
  if (daysLeft === 0) return t('expiresToday');
  if (daysLeft > 0) return tn('expiresInDays', daysLeft);
  return tn('expiredDaysAgo', -daysLeft);
}
