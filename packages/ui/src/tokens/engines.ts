// SPDX-License-Identifier: Apache-2.0
import type { EngineId } from '../components/engine/engineGlyphs';

/** Engine tile colours: the tile fill and the glyph ink (DESIGN.md §4). */
export const engineTiles: Readonly<
  Record<EngineId, { bg: string; ink: string }>
> = {
  postgres: { bg: '#3D6F9E', ink: '#FFF' },
  mysql: { bg: '#1F7A94', ink: '#FFF' },
  mariadb: { bg: '#A8623F', ink: '#FFF' },
  sqlserver: { bg: '#B8322C', ink: '#FFF' },
  oracle: { bg: '#C2452F', ink: '#FFF' },
  cockroachdb: { bg: '#6A41F0', ink: '#FFF' },
  greenplum: { bg: '#1F9A57', ink: '#FFF' },
  redshift: { bg: '#7D4EE6', ink: '#FFF' },
  snowflake: { bg: '#2AA7DC', ink: '#FFF' },
  bigquery: { bg: '#4A82EE', ink: '#FFF' },
  clickhouse: { bg: '#F5C518', ink: '#1B1A17' },
  vertica: { bg: '#2B6FE8', ink: '#FFF' },
  duckdb: { bg: '#F2DF2C', ink: '#1B1A17' },
  sqlite: { bg: '#2E86C8', ink: '#FFF' },
  'libsql-file': { bg: '#1C3F3A', ink: '#5FF0CF' },
  libsql: { bg: '#1C3F3A', ink: '#5FF0CF' },
  d1: { bg: '#E8792B', ink: '#FFF' },
  dynamodb: { bg: '#4860D8', ink: '#FFF' },
  mongodb: { bg: '#1F9D57', ink: '#FFF' },
  redis: { bg: '#CF3A2E', ink: '#FFF' },
  cassandra: { bg: '#2389B5', ink: '#FFF' },
  elasticsearch: { bg: '#16B3A6', ink: '#FFF' },
};
