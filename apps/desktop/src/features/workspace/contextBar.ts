// SPDX-License-Identifier: Apache-2.0
import type { InsecureReasonDto, ProfileDto } from '@openrows/ipc';
import { insecureReasons, statusLabel } from '../connections';
import type { SessionStatus } from '../connections';

/** Segments of the toolbar context bar, TablePlus style. */
export type ContextSegments = {
  /** Environment tag, e.g. `LOCAL`; empty without a connection. */
  env: string;
  /** `PostgreSQL 14.17` once connected, else the connection state. */
  engine: string;
  name: string;
  /** Active database; empty when none is open. */
  database: string;
  /** `schema.object` of the active object tab, else the selected schema. */
  object: string;
  /** Why the active connection is not secure; empty when it is. */
  insecure: InsecureReasonDto[];
};

/**
 * `ENV | PostgreSQL 14.17 : connection : database : schema.object`, from the
 * active connection, its status, database and the active tab's object.
 */
export function contextSegments(input: {
  profile: ProfileDto | undefined;
  status: SessionStatus | undefined;
  database: string;
  schema: string;
  objectPath: readonly string[] | undefined;
}): ContextSegments {
  const { profile, status } = input;
  if (!profile || !status)
    return {
      env: '',
      engine: 'Not connected',
      name: 'No connection',
      database: '',
      object: '',
      insecure: [],
    };
  const env = (profile.env.label ?? profile.env.kind).toUpperCase();
  const engine =
    status.state === 'connected'
      ? `${status.server.product} ${status.server.serverVersion}`
      : statusLabel(status);
  const object = input.database
    ? (input.objectPath?.join('.') ?? input.schema)
    : '';
  return {
    env,
    engine,
    name: profile.name,
    database: input.database,
    object,
    insecure: insecureReasons(profile, status),
  };
}
