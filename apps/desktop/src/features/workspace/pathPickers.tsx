// SPDX-License-Identifier: Apache-2.0
import { EngineIcon, EnvBadge, Icon, isEngineId } from '@openrows/ui';
import type { PickerItem, PickerSection } from '@openrows/ui';
import type { EnvDto, ObjectDto, ProfileDto } from '@openrows/ipc';
import type { Connections, SessionStatus } from '../connections';
import type { Catalog } from '../schema';
import { objectGroups } from '../schema/catalog';
import { OBJECT_PAGE } from '../schema/useCatalog';
import { t } from './navMessages';

/** The picker item that opens the connection flow. */
export const NEW_CONNECTION = '+new';

/** Relations the object picker lists, by catalog section. */
export const PICKER_GROUPS = objectGroups.filter((group) =>
  ['tables', 'views', 'materializedViews'].includes(group.id),
);

const STANDARD = new Set(['local', 'dev', 'staging', 'prod']);

/** The environment tag of a row or the bar. */
export function EnvTag(props: {
  env: EnvDto;
  className?: string | undefined;
}): React.JSX.Element {
  const { env } = props;
  if (STANDARD.has(env.kind) && !env.label)
    return (
      <EnvBadge
        environment={env.kind as 'local' | 'dev' | 'staging' | 'prod'}
        className={props.className}
      />
    );
  return <span className={props.className}>{env.label ?? env.kind}</span>;
}

/** An engine tile, or a database glyph for engines without one. */
export function EngineTile(props: {
  engine: string;
  size: 'sm' | 'md';
}): React.JSX.Element {
  return isEngineId(props.engine) ? (
    <EngineIcon engine={props.engine} size={props.size} />
  ) : (
    <Icon name="database" />
  );
}

function statusOf(status: SessionStatus): NonNullable<PickerItem['status']> {
  if (status.state === 'connected') return 'on';
  if (status.state === 'connecting') return 'warn';
  return status.state === 'failed' ? 'off' : 'idle';
}

function whereOf(profile: ProfileDto): string {
  if (profile.file) return profile.file.path || t('inMemory');
  return `${profile.host}:${String(profile.port)}`;
}

/** Connections by their group, then New connection… (DESIGN.md §6). */
export function connectionSections(
  connections: Connections,
  current: string | undefined,
): PickerSection[] {
  const groups = new Map<string, PickerItem[]>();
  for (const profile of connections.profiles) {
    const head = profile.group ?? t('connections');
    const items = groups.get(head) ?? [];
    items.push({
      id: profile.id,
      name: profile.name,
      sub: whereOf(profile),
      lead: <EngineTile engine={profile.engine} size="md" />,
      meta: <EnvTag env={profile.env} />,
      status: statusOf(connections.statusOf(profile.id)),
      current: profile.id === current,
    });
    groups.set(head, items);
  }
  return [
    ...[...groups].map(([head, items]) => ({ head, items })),
    {
      items: [
        {
          id: NEW_CONNECTION,
          name: t('newConnectionItem'),
          lead: <Icon name="plus" />,
        },
      ],
    },
  ];
}

/** The connection's databases; opened ones say so. */
export function databaseSections(
  catalog: Catalog,
  current: string,
): PickerSection[] {
  const { databases } = catalog;
  const names =
    databases.status === 'ready'
      ? databases.data
          .filter((item) => !item.system || item.name === current)
          .map((item) => item.name)
      : catalog.opened;
  const items = names.map((name) => ({
    id: name,
    name,
    lead: <Icon name="database" />,
    meta: catalog.opened.includes(name) ? t('open') : undefined,
    current: name === current,
  }));
  return [{ head: t('databases'), items }];
}

/** The database's schemas, system schemas only when shown in the tree. */
export function schemaSections(
  catalog: Catalog,
  current: string,
): PickerSection[] {
  const { schemas } = catalog;
  if (schemas.status !== 'ready') return [{ head: t('loading'), items: [] }];
  const items = schemas.data
    .filter(
      (item) => catalog.systemSchemas || !item.system || item.name === current,
    )
    .map((item) => ({
      id: item.name,
      name: item.name,
      lead: <Icon name="schema" />,
      current: item.name === current,
    }));
  return [{ head: t('schemas'), items }];
}

/** `1.2M rows` from a planner estimate. */
function rowsOf(object: ObjectDto): string | undefined {
  const rows = object.rowEstimate;
  if (rows === null || rows < 0) return undefined;
  const short = new Intl.NumberFormat(undefined, {
    notation: 'compact',
  }).format(rows);
  return t('rows', { rows: short });
}

/** Tables, views and materialized views of a schema. */
export function objectSections(
  catalog: Catalog,
  schema: string,
  current: string | undefined,
): PickerSection[] {
  return PICKER_GROUPS.flatMap((group) => {
    const page = catalog.objects(group, OBJECT_PAGE, schema);
    if (page.status !== 'ready')
      return group.id === 'tables' ? [{ head: t('loading'), items: [] }] : [];
    const items = page.data.objects
      .filter((object) => object.schema === schema)
      .map((object) => ({
        id: `${object.kind}\n${object.name}`,
        name: object.name,
        lead: (
          <Icon
            name={
              object.kind === 'table' || object.kind === 'foreignTable'
                ? 'table'
                : 'eye'
            }
          />
        ),
        meta: rowsOf(object),
        current: object.name === current,
      }));
    return items.length ? [{ head: group.label, items }] : [];
  });
}
