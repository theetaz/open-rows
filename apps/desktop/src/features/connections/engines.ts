// SPDX-License-Identifier: Apache-2.0
import type { EngineDto, ProfileDto } from '@openrows/ipc';

/** One tile of the connection-type picker (the TablePlus connection types). */
export type EngineChoice = {
  /** Stable id; equals the stored engine for implemented types. */
  id: string;
  name: string;
  /** Two letters on the neutral tile (no vendor logos). */
  monogram: string;
  /** The engine profiles store, or `null` while no driver serves it. */
  engine: EngineDto | null;
};

/** Picker order, row by row, as in TablePlus. */
export const ENGINE_CHOICES: readonly EngineChoice[] = [
  { id: 'postgres', name: 'PostgreSQL', monogram: 'Pg', engine: 'postgres' },
  { id: 'redshift', name: 'Amazon Redshift', monogram: 'Rs', engine: null },
  { id: 'mysql', name: 'MySQL', monogram: 'My', engine: 'mysql' },
  { id: 'mariadb', name: 'MariaDB', monogram: 'Ma', engine: 'mariadb' },
  { id: 'sqlserver', name: 'SQL Server', monogram: 'Ss', engine: null },
  { id: 'cassandra', name: 'Cassandra', monogram: 'Cs', engine: null },
  { id: 'clickhouse', name: 'ClickHouse', monogram: 'Ch', engine: null },
  { id: 'bigquery', name: 'BigQuery', monogram: 'Bq', engine: null },
  { id: 'dynamodb', name: 'DynamoDB', monogram: 'Dy', engine: null },
  { id: 'elasticsearch', name: 'Elasticsearch', monogram: 'Es', engine: null },
  { id: 'libsql', name: 'libSQL', monogram: 'Ls', engine: null },
  { id: 'd1', name: 'Cloudflare D1', monogram: 'D1', engine: null },
  { id: 'mongodb', name: 'MongoDB', monogram: 'Mg', engine: null },
  { id: 'snowflake', name: 'Snowflake', monogram: 'Sf', engine: null },
  { id: 'redis', name: 'Redis', monogram: 'Re', engine: null },
  { id: 'sqlite', name: 'SQLite', monogram: 'Sl', engine: 'sqlite' },
  { id: 'duckdb', name: 'DuckDB', monogram: 'Du', engine: null },
  { id: 'oracle', name: 'Oracle', monogram: 'Or', engine: null },
  { id: 'cockroachdb', name: 'CockroachDB', monogram: 'Cr', engine: null },
  { id: 'greenplum', name: 'Greenplum', monogram: 'Gp', engine: null },
  { id: 'vertica', name: 'Vertica', monogram: 'Ve', engine: null },
];

/** Display name of a stored engine, e.g. `MariaDB`. */
export function engineName(engine: EngineDto): string {
  return (
    ENGINE_CHOICES.find((choice) => choice.engine === engine)?.name ?? engine
  );
}

/** Default port of a server engine; `null` for local files. */
export function defaultPort(engine: EngineDto): number | null {
  switch (engine) {
    case 'postgres':
      return 5432;
    case 'mysql':
    case 'mariadb':
      return 3306;
    case 'sqlite':
      return null;
  }
}

/** Engines that open a local file instead of a server. */
export function isFileEngine(engine: EngineDto): boolean {
  return engine === 'sqlite';
}

/** URL scheme that names `engine` (composing URLs). */
export function urlScheme(engine: EngineDto): string {
  return engine === 'postgres' ? 'postgres' : engine;
}

/** Choices whose name, id or monogram contains every typed term. */
export function matchingChoices(query: string): EngineChoice[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  return ENGINE_CHOICES.filter((choice) => {
    const text = `${choice.name} ${choice.id} ${choice.monogram}`.toLowerCase();
    return terms.every((term) => text.includes(term));
  });
}

/** `MySQL · db.internal:3306` or `SQLite · /data/app.sqlite`. */
export function profileLocation(profile: ProfileDto): string {
  const place = profile.file
    ? profile.file.path
    : `${profile.host}:${String(profile.port)}`;
  return `${engineName(profile.engine)} · ${place}`;
}
