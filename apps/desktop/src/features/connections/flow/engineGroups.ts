// SPDX-License-Identifier: Apache-2.0
import { defaultPort, ENGINE_CHOICES, matchingChoices } from '../engines';
import type { EngineChoice } from '../engines';
import { t } from './messages';

/** The prototype's five families of engine tiles, in picker order. */
const GROUPS: readonly { title: string; ids: readonly string[] }[] = [
  {
    title: t('groupRelational'),
    ids: ['postgres', 'mysql', 'mariadb', 'sqlserver', 'oracle', 'cockroachdb'],
  },
  { title: t('groupFiles'), ids: ['duckdb', 'sqlite', 'libsql-file'] },
  {
    title: t('groupAnalytics'),
    ids: [
      'greenplum',
      'redshift',
      'snowflake',
      'bigquery',
      'clickhouse',
      'vertica',
    ],
  },
  {
    title: t('groupDocument'),
    ids: ['mongodb', 'redis', 'dynamodb', 'cassandra', 'elasticsearch'],
  },
  { title: t('groupEdge'), ids: ['libsql', 'd1'] },
];

const FILE_EXTENSIONS: Partial<Record<string, string>> = {
  duckdb: '.duckdb',
  sqlite: '.sqlite',
  'libsql-file': '.db',
};

type EngineGroup = { title: string; choices: EngineChoice[] };

/** Groups of the choices matching `query`; empty groups are left out. */
export function engineGroups(query: string): EngineGroup[] {
  const visible = new Set(matchingChoices(query).map((choice) => choice.id));
  return GROUPS.map((group) => ({
    title: group.title,
    choices: group.ids
      .filter((id) => visible.has(id))
      .map((id) => ENGINE_CHOICES.find((choice) => choice.id === id))
      .filter((choice): choice is EngineChoice => choice !== undefined),
  })).filter((group) => group.choices.length > 0);
}

/** The engine count shown in the search placeholder. */
export const ENGINE_COUNT = ENGINE_CHOICES.length;

/** The tile's second line: its file type, default port, or "Coming soon". */
export function tileDetail(choice: EngineChoice): string {
  if (!choice.engine) return t('comingSoon');
  const extension = FILE_EXTENSIONS[choice.id];
  if (extension) return `${t('fileKind')} · ${extension}`;
  const port = defaultPort(choice.engine);
  return port === null
    ? t('cloudService')
    : t('portNumber', { port: String(port) });
}
