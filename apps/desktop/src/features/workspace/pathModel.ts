// SPDX-License-Identifier: Apache-2.0
import type { IconName } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import type { QueryTab } from '../query/tabs';
import { t } from './navMessages';

/** A level of the address; each opens its own picker. */
export type PathLevel = 'connection' | 'database' | 'schema' | 'object';

/** One segment of the path bar. */
export type PathSegment = {
  level: PathLevel;
  text: string;
  icon?: IconName;
  /** The connection's engine tile instead of an icon. */
  engine?: string;
  /** Tooltip when it says more than the text (the connection's state). */
  title?: string;
  /** `last`: the current object; `todo`: the path stops short; `prompt`: nothing chosen. */
  role?: 'last' | 'todo' | 'prompt';
};

/** What the address is built from. */
type PathInput = {
  tab: QueryTab | undefined;
  profile: ProfileDto | undefined;
  /** The browsed database and schema when the tab does not name them. */
  database: string;
  schema: string;
  /** The connection's state, e.g. `Connected · PostgreSQL 17.2`. */
  status?: string;
};

const OBJECT_ICONS: Partial<Record<string, IconName>> = {
  view: 'eye',
  materializedView: 'eye',
  foreignTable: 'table',
};

/** The last segment: the tab's object, its own title, or "Choose a table…". */
function lastSegment(tab: QueryTab | undefined): PathSegment {
  if (tab?.kind === 'table' && tab.table)
    return {
      level: 'object',
      text: tab.table.name,
      icon: OBJECT_ICONS[tab.table.kind ?? 'table'] ?? 'table',
      role: 'last',
    };
  if (tab?.kind === 'inspection' && tab.objectPath?.[1])
    return {
      level: 'object',
      text: tab.objectPath[1],
      icon: 'eye',
      role: 'last',
    };
  if (tab && tab.kind !== 'connection') {
    const icon: IconName =
      tab.kind === 'query'
        ? 'terminal'
        : tab.kind === 'monitor'
          ? 'activity'
          : 'list';
    return { level: 'object', text: tab.title, icon, role: 'last' };
  }
  return { level: 'object', text: t('chooseTable'), role: 'todo' };
}

/** The schema the tab shows: its table's, its object's, else the browsed one. */
function schemaOf(tab: QueryTab | undefined, schema: string): string {
  if (tab?.kind === 'table' && tab.table) return tab.table.schema;
  if (tab?.kind === 'inspection' && tab.objectPath?.[0])
    return tab.objectPath[0];
  return schema;
}

/**
 * `connection › database › schema › object` for the shown tab. The
 * connection flow shows its own title; without a connection the bar asks
 * for one.
 */
export function pathSegments(input: PathInput): PathSegment[] {
  const { tab, profile } = input;
  if (tab?.kind === 'connection')
    return [
      { level: 'connection', text: tab.title, icon: 'plug', role: 'last' },
    ];
  if (!profile)
    return [
      {
        level: 'connection',
        text: t('chooseConnection'),
        icon: 'search',
        role: 'prompt',
      },
    ];
  const database = tab?.database || input.database;
  const segments: PathSegment[] = [
    {
      level: 'connection',
      text: profile.name,
      engine: profile.engine,
      ...(input.status ? { title: `${profile.name} · ${input.status}` } : {}),
    },
  ];
  if (!database)
    return [
      ...segments,
      { level: 'database', text: t('chooseDatabase'), role: 'todo' },
    ];
  segments.push({ level: 'database', text: database, icon: 'database' });
  const schema = schemaOf(tab, input.schema);
  const utility = tab?.kind === 'processes' || tab?.kind === 'monitor';
  if (schema && !utility)
    segments.push({ level: 'schema', text: schema, icon: 'schema' });
  return [...segments, lastSegment(tab)];
}
