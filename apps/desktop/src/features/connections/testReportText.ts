// SPDX-License-Identifier: Apache-2.0

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
  const days = (count: number) =>
    `${String(count)} day${count === 1 ? '' : 's'}`;
  if (daysLeft === 0) return 'expires today';
  if (daysLeft > 0) return `expires in ${days(daysLeft)}`;
  return `expired ${days(-daysLeft)} ago`;
}
