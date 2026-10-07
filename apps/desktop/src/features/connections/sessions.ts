// SPDX-License-Identifier: Apache-2.0
import type { SessionStateChanged } from '@openrows/ipc';

/** What the UI shows for one saved connection's session. */
export type SessionStatus =
  | { state: 'disconnected' }
  | { state: 'connecting' }
  | { state: 'connected'; sessionId: string; serverVersion: string }
  | { state: 'failed'; message: string };

export type Statuses = Readonly<Record<string, SessionStatus | undefined>>;

export const disconnected: SessionStatus = { state: 'disconnected' };

/** Apply a core session event to the profile that owns the session. */
export function applySessionEvent(
  statuses: Statuses,
  event: SessionStateChanged,
): Statuses {
  const owner = Object.entries(statuses).find(
    ([, status]) =>
      status?.state === 'connected' && status.sessionId === event.sessionId,
  )?.[0];
  if (owner === undefined) return statuses;
  if (event.state === 'failed')
    return {
      ...statuses,
      [owner]: {
        state: 'failed',
        message: event.message ?? 'The connection was lost',
      },
    };
  if (event.state === 'closed') return { ...statuses, [owner]: disconnected };
  return statuses;
}

/** Short status text for the sidebar header, toolbar and rail tooltip. */
export function statusLabel(status: SessionStatus): string {
  switch (status.state) {
    case 'connecting':
      return 'Connecting…';
    case 'connected':
      return status.serverVersion
        ? `Connected · PostgreSQL ${status.serverVersion}`
        : 'Connected';
    case 'failed':
      return 'Connection failed';
    case 'disconnected':
      return 'Not connected';
  }
}
