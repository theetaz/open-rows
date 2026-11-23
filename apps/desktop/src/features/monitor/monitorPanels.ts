// SPDX-License-Identifier: Apache-2.0
import type { IconName } from '@openrows/ui';
import type { EngineDto } from '@openrows/ipc';
import { CLICKHOUSE_HIDDEN, CLICKHOUSE_LABELS } from './clickhouseMetrics';
import { t } from './messages/lazyDashboard';

const PANELS = [
  { value: 'overview', label: t('overview'), icon: 'activity' },
  { value: 'incidents', label: t('incidents'), icon: 'bell' },
  { value: 'activity', label: t('sessions'), icon: 'list' },
  { value: 'blocking', label: t('blocking'), icon: 'lock' },
  { value: 'statements', label: t('topQueries'), icon: 'terminal' },
  { value: 'advisor', label: t('advisor'), icon: 'chart' },
  { value: 'tables', label: t('tables'), icon: 'table' },
  { value: 'indexes', label: t('indexes'), icon: 'key' },
  { value: 'replication', label: t('replication'), icon: 'server' },
  { value: 'io', label: t('iO'), icon: 'database' },
  { value: 'assistant', label: t('askAssistant'), icon: 'sparkles' },
  { value: 'boards', label: t('boards'), icon: 'columns' },
] as const satisfies readonly {
  value: string;
  label: string;
  icon: IconName;
}[];
/** One Monitor section: its id, its engine-specific label and its icon. */
export type Panel = (typeof PANELS)[number]['value'];
export type PanelOption = { value: Panel; label: string; icon: IconName };
/** MySQL/MariaDB table, index and file sources are instrumented I/O summaries. */
const MYSQL_LABELS: Partial<Record<Panel, string>> = {
  tables: t('tableIO'),
  indexes: t('indexIO'),
  io: t('fileIO'),
};
/** SQL Server tables/indexes are usage DMVs; the I/O slot carries wait statistics. */
const SQLSERVER_LABELS: Partial<Record<Panel, string>> = {
  tables: t('tableUsage'),
  indexes: t('indexUsage'),
  io: t('waits'),
};
/** Embedded and remote libSQL see only OpenRows' own statements. */
const EMBEDDED_PANELS: ReadonlySet<Panel> = new Set([
  'overview',
  'incidents',
  'assistant',
  'statements',
]);
export const ENGINE_NAMES: Partial<Record<EngineDto, string>> = {
  postgres: 'PostgreSQL',
  mysql: 'MySQL',
  mariadb: 'MariaDB',
  sqlserver: 'SQL Server',
  clickhouse: 'ClickHouse',
  sqlite: 'SQLite',
  'libsql-file': 'libSQL file',
  libsql: 'libSQL',
  duckdb: 'DuckDB',
};

function relabel(
  labels: Partial<Record<Panel, string>>,
): (panel: PanelOption) => PanelOption {
  return (panel) => ({ ...panel, label: labels[panel.value] ?? panel.label });
}

/** The sections an engine's collector can fill, under its own names. */
export function panels(engine: EngineDto): readonly PanelOption[] {
  if (engine === 'clickhouse')
    return PANELS.filter((panel) => !CLICKHOUSE_HIDDEN.has(panel.value)).map(
      relabel(CLICKHOUSE_LABELS),
    );
  if (ownSessionsOnly(engine))
    return PANELS.filter((panel) => EMBEDDED_PANELS.has(panel.value)).map(
      relabel({ statements: t('ownQueries') }),
    );
  const labels =
    engine === 'mysql' || engine === 'mariadb'
      ? MYSQL_LABELS
      : engine === 'sqlserver'
        ? SQLSERVER_LABELS
        : {};
  return PANELS.map(relabel(labels));
}

/** Engines whose collector reads only OpenRows' own statements. */
function ownSessionsOnly(engine: EngineDto | undefined): boolean {
  return (
    engine === 'sqlite' ||
    engine === 'libsql-file' ||
    engine === 'libsql' ||
    engine === 'duckdb'
  );
}

/** The section and row an advisor evidence link points at. */
export function evidenceTarget(
  link: string,
): { panel: Panel; identity: string } | null {
  const [source, ...identity] = link.split(':');
  if (source === 'sessions')
    return { panel: 'activity', identity: identity.join(':') };
  if (source === 'tables' || source === 'indexes' || source === 'statements')
    return { panel: source, identity: identity.join(':') };
  return null;
}
