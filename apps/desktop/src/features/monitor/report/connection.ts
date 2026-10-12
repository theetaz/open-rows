// SPDX-License-Identifier: Apache-2.0
import type { EngineDto, ProfileDto } from '@openrows/ipc';

/** Report metadata: profile name, engine and selected database. */
export type Connection = { name: string; engine: EngineDto; database: string };

export function reportConnection(
  profile: ProfileDto,
  database: string,
): Connection {
  return { name: profile.name, engine: profile.engine, database };
}
