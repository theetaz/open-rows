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
  /** The connection was lost; the core is opening a new one (queries wait). */
  | { name: string; state: 'reconnecting'; sessionId: string; message: string }
  /** `lost`: it was open and the connection could not be restored. */
  | { name: string; state: 'failed'; message: string; lost?: true };

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
type StatusTone =
  'connected' | 'connecting' | 'reconnecting' | 'failed' | 'none';

/** How the live sessions of a connected profile are doing. */
function health(status: SessionStatus): 'up' | 'reconnecting' | 'lost' {
  if (status.state !== 'connected') return 'up';
  const { databases } = status;
  if (databases.some((item) => item.state === 'reconnecting'))
    return 'reconnecting';
  return databases.some((item) => item.state === 'failed' && item.lost)
    ? 'lost'
    : 'up';
}

/**
 * Rail dot: green connected, amber connecting or reconnecting, red failed or
 * a database whose connection was lost, none otherwise.
 */
export function statusTone(status: SessionStatus): StatusTone {
  if (status.state === 'disconnected') return 'none';
  if (status.state !== 'connected') return status.state;
  const now = health(status);
  if (now === 'up') return 'connected';
  return now === 'lost' ? 'failed' : 'reconnecting';
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
      (item) => liveSession(item) === sessionId,
    );
    if (database) return [profileId, database];
  }
  return undefined;
}

/** The session of an open or reconnecting database. */
function liveSession(database: OpenedDatabase): string | null {
  return database.state === 'open' || database.state === 'reconnecting'
    ? database.sessionId
    : null;
}

/** The database after a core session event; `null` removes it. */
function afterEvent(
  database: OpenedDatabase & { sessionId: string },
  event: SessionStateChanged,
): OpenedDatabase | null {
  const { name, sessionId } = database;
  const message = event.message ?? 'The connection was lost';
  switch (event.state) {
    case 'failed':
      return { name, state: 'failed', message, lost: true };
    case 'closed':
      return null;
    case 'reconnecting':
      return { name, state: 'reconnecting', sessionId, message };
    case 'idle':
    case 'busy':
      return { name, state: 'open', sessionId };
    case 'connecting':
      return database;
  }
}

/** Apply a core session event to the opened database that owns the session. */
export function applySessionEvent(
  statuses: Statuses,
  event: SessionStateChanged,
): Statuses {
  const owner = ownerOf(statuses, event.sessionId);
  if (!owner) return statuses;
  const [profileId, database] = owner;
  const sessionId = liveSession(database);
  if (!sessionId) return statuses;
  const next = afterEvent({ ...database, sessionId }, event);
  if (!next) return removeDatabase(statuses, profileId, database.name);
  return next.state === database.state
    ? statuses
    : setDatabase(statuses, profileId, next);
}

/** The opened `database` of a connected profile. */
export function openedDatabase(
  status: SessionStatus,
  database: string,
): OpenedDatabase | undefined {
  if (status.state !== 'connected') return undefined;
  return status.databases.find((item) => item.name === database);
}

/** The session a query on `database` runs on, when that database is open. */
export function sessionOf(
  status: SessionStatus,
  database: string,
): string | null {
  const opened = openedDatabase(status, database);
  return opened ? liveSession(opened) : null;
}

/** Short status text for the sidebar header, toolbar and rail tooltip. */
export function statusLabel(status: SessionStatus): string {
  switch (status.state) {
    case 'connecting':
      return 'Connecting…';
    case 'connected':
      return connectedLabel(status);
    case 'failed':
      return 'Connection failed';
    case 'disconnected':
      return 'Not connected';
  }
}

function connectedLabel(
  status: Extract<SessionStatus, { state: 'connected' }>,
): string {
  const now = health(status);
  if (now === 'reconnecting') return 'Reconnecting…';
  if (now === 'lost') return 'Connection lost';
  return status.server.serverVersion
    ? `Connected · ${serverLabel(status.server)}`
    : 'Connected';
}

/** Product, version and flavour, e.g. `PostgreSQL 17.2 · Neon`. */
export function serverLabel(server: ServerDto): string {
  const product = `${server.product} ${server.serverVersion}`;
  return server.flavour ? `${product} · ${server.flavour}` : product;
}
