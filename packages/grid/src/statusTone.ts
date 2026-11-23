// SPDX-License-Identifier: Apache-2.0
/** How a status-like enum value reads: done, failed, in flight or waiting. */
export type StatusTone = 'ok' | 'bad' | 'run' | 'wait';

const WORDS: Readonly<Record<string, StatusTone>> = {
  success: 'ok',
  succeeded: 'ok',
  ok: 'ok',
  done: 'ok',
  complete: 'ok',
  completed: 'ok',
  active: 'ok',
  enabled: 'ok',
  healthy: 'ok',
  passed: 'ok',
  approved: 'ok',
  resolved: 'ok',
  shipped: 'ok',
  delivered: 'ok',
  failed: 'bad',
  failure: 'bad',
  error: 'bad',
  errored: 'bad',
  rejected: 'bad',
  cancelled: 'bad',
  canceled: 'bad',
  refunded: 'bad',
  expired: 'bad',
  blocked: 'bad',
  disabled: 'bad',
  running: 'run',
  processing: 'run',
  in_progress: 'run',
  started: 'run',
  sent: 'run',
  paid: 'run',
  queued: 'wait',
  pending: 'wait',
  waiting: 'wait',
  scheduled: 'wait',
  paused: 'wait',
  retrying: 'wait',
  draft: 'wait',
};

/** Labels already looked up, so painting a frame allocates no strings. */
const seen = new Map<string, StatusTone | null>();
const SEEN_LIMIT = 512;

/**
 * The tone of an enum label that names a state (`SUCCESS`, `failed`,
 * `in-progress`…), or `null` for any other label, which paints plainly.
 */
export function statusTone(label: string): StatusTone | null {
  const known = seen.get(label);
  if (known !== undefined) return known;
  const tone =
    label.length > 16
      ? null
      : (WORDS[label.toLowerCase().replace(/[\s-]/g, '_')] ?? null);
  if (seen.size < SEEN_LIMIT) seen.set(label, tone);
  return tone;
}
