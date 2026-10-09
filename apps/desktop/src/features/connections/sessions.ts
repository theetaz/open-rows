// SPDX-License-Identifier: Apache-2.0
import type {
  InsecureReasonDto,
  ServerDto,
  SessionStateChanged,
} from '@openrows/ipc';

/**
 * One opened database of a connection. PostgreSQL cannot switch databases on
 * a connection, so each opened database has its own user session.
 */
export type OpenedDatabase =
  | { name: string; state: 'opening' }
  | { name: string; state: 'open'; sessionId: string }
  | { name: string; state: 'failed'; message: string };

/** What the UI shows for one saved connection. */
export type SessionStatus =
  | { state: 'disconnected' }
  | { state: 'connecting' }
  | {
      state: 'connected';
      /** Server info read on the home database (`server.database`). */
      server: ServerDto;
      /** Why this live connection is insecure (TLS, a host key not saved). */
      insecure: readonly InsecureReasonDto[];
      /** Opened databases in opening order. */
      databases: readonly OpenedDatabase[];
      /** Changes on every connect, so caches of an earlier connect are not reused. */
      generation: number;
    }
  | { state: 'failed'; message: string };

export type Statuses = Readonly<Record<string, SessionStatus | undefined>>;

export const disconnected: SessionStatus = { state: 'disconnected' };

/** Colour state of a rail tile's status dot. */
type StatusTone = 'connected' | 'connecting' | 'failed' | 'none';

/** Rail dot: green connected, amber connecting, red failed, none otherwise. */
export function statusTone(status: SessionStatus): StatusTone {
  if (status.state === 'disconnected') return 'none';
  return status.state;
}

/** Replace the opened databases of a connected profile. */
function withDatabases(
  statuses: Statuses,
  profileId: string,
  change: (databases: readonly OpenedDatabase[]) => readonly OpenedDatabase[],
): Statuses {
  const status = statuses[profileId];
  if (status?.state !== 'connected') return statuses;
  return {
    ...statuses,
    [profileId]: { ...status, databases: change(status.databases) },
  };
}

/** Add or replace one opened database, keeping its position. */
export function setDatabase(
  statuses: Statuses,
  profileId: string,
  database: OpenedDatabase,
): Statuses {
  return withDatabases(statuses, profileId, (databases) =>
    databases.some(({ name }) => name === database.name)
      ? databases.map((item) => (item.name === database.name ? database : item))
      : [...databases, database],
  );
}

/** Forget one opened database. */
export function removeDatabase(
  statuses: Statuses,
  profileId: string,
  name: string,
): Statuses {
  return withDatabases(statuses, profileId, (databases) =>
    databases.filter((item) => item.name !== name),
  );
}

/** The opened database whose session is `sessionId`, with its profile. */
function ownerOf(
  statuses: Statuses,
  sessionId: string,
): [string, OpenedDatabase] | undefined {
  for (const [profileId, status] of Object.entries(statuses)) {
    if (status?.state !== 'connected') continue;
    const database = status.databases.find(
      (item) => item.state === 'open' && item.sessionId === sessionId,
    );
    if (database) return [profileId, database];
  }
  return undefined;
}

/** Apply a core session event to the opened database that owns the session. */
export function applySessionEvent(
  statuses: Statuses,
  event: SessionStateChanged,
): Statuses {
  const owner = ownerOf(statuses, event.sessionId);
  if (!owner) return statuses;
  const [profileId, { name }] = owner;
  if (event.state === 'failed')
    return setDatabase(statuses, profileId, {
      name,
      state: 'failed',
      message: event.message ?? 'The connection was lost',
    });
  if (event.state === 'closed')
    return removeDatabase(statuses, profileId, name);
  return statuses;
}

/** The session a query on `database` runs on, when that database is open. */
export function sessionOf(
  status: SessionStatus,
  database: string,
): string | null {
  if (status.state !== 'connected') return null;
  const opened = status.databases.find((item) => item.name === database);
  return opened?.state === 'open' ? opened.sessionId : null;
}

/** Short status text for the sidebar header, toolbar and rail tooltip. */
export function statusLabel(status: SessionStatus): string {
  switch (status.state) {
    case 'connecting':
      return 'Connecting…';
    case 'connected':
      return status.server.serverVersion
        ? `Connected · ${serverLabel(status.server)}`
        : 'Connected';
    case 'failed':
      return 'Connection failed';
    case 'disconnected':
      return 'Not connected';
  }
}

/** Product, version and flavour, e.g. `PostgreSQL 17.2 · Neon`. */
export function serverLabel(server: ServerDto): string {
  const product = `${server.product} ${server.serverVersion}`;
  return server.flavour ? `${product} · ${server.flavour}` : product;
}
