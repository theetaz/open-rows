// SPDX-License-Identifier: Apache-2.0
import type { FilterDto, ObjectKindDto, ProfileDto } from '@openrows/ipc';

/**
 * `openrows://` links (FR-APP-09, docs/16 §3.1). Links name the saved
 * connection by id and name only: never a host, user, password or SQL. The
 * native parser accepts exactly what these functions write.
 */
const SCHEME = 'openrows://';

/** Relations a link can open as a data view. */
const relations = new Set<ObjectKindDto>([
  'table',
  'view',
  'materializedView',
  'foreignTable',
]);

/** Whether `kind` can be the object of an object or table link. */
export function isLinkable(kind: ObjectKindDto): boolean {
  return relations.has(kind);
}

function connectionParams(
  profile: Pick<ProfileDto, 'id' | 'name'>,
): URLSearchParams {
  const params = new URLSearchParams();
  params.append('id', profile.id);
  if (profile.name) params.append('name', profile.name);
  return params;
}

/** Copy Link of a saved connection, optionally on one database. */
export function connectionLink(
  profile: Pick<ProfileDto, 'id' | 'name'>,
  database?: string,
): string {
  const params = connectionParams(profile);
  if (database) params.append('database', database);
  return `${SCHEME}connection?${params.toString()}`;
}

/** A schema object: a schema (no `name`) or a relation of `kind`. */
type LinkTarget = {
  database: string;
  schema: string;
  name?: string;
  kind?: ObjectKindDto;
};

function objectParams(
  profile: Pick<ProfileDto, 'id' | 'name'>,
  target: LinkTarget,
): URLSearchParams {
  const params = connectionParams(profile);
  params.append('database', target.database);
  params.append('schema', target.schema);
  if (target.name) {
    params.append('object', target.name);
    params.append('kind', target.kind ?? 'table');
  }
  return params;
}

/** Copy Link of a schema or relation. */
export function objectLink(
  profile: Pick<ProfileDto, 'id' | 'name'>,
  target: LinkTarget,
): string {
  return `${SCHEME}object?${objectParams(profile, target).toString()}`;
}

/** Copy Link of a relation with its filters, which must all match. */
export function tableLink(
  profile: Pick<ProfileDto, 'id' | 'name'>,
  target: LinkTarget & { name: string },
  filters: readonly FilterDto[],
): string {
  const params = objectParams(profile, target);
  for (const filter of filters)
    params.append(
      'filter',
      new URLSearchParams({
        column: filter.column,
        op: filter.operator,
        value: filter.value,
        second: filter.second,
      }).toString(),
    );
  return `${SCHEME}table?${params.toString()}`;
}

/**
 * The applied filters a link can carry: parameterised comparisons that must
 * all match. Raw SQL predicates and OR groups cannot be linked (`null`).
 */
export function linkableFilters(view: FilterView): FilterDto[] | null {
  const groups = view.appliedGroups ?? [
    { combinator: view.combinator, filters: view.applied },
  ];
  const filters = groups.flatMap((group) => group.filters);
  const ors =
    groups.some(
      (group) => group.combinator === 'or' && group.filters.length > 1,
    ) ||
    (view.appliedGroups !== undefined &&
      view.combinator === 'or' &&
      groups.filter((group) => group.filters.length > 0).length > 1);
  if (ors || filters.length > 16) return null;
  if (filters.some((filter) => filter.operator === 'raw')) return null;
  return [...filters];
}

/** The applied filters of a table view. */
type FilterView = {
  applied: readonly FilterDto[];
  combinator: 'and' | 'or';
  appliedGroups?:
    | readonly { combinator: 'and' | 'or'; filters: readonly FilterDto[] }[]
    | undefined;
};

/** Copy Link of a table tab with its applied filters; `null` when they cannot be linked. */
export function filterLink(
  profile: Pick<ProfileDto, 'id' | 'name'>,
  table: {
    database: string;
    schema: string;
    name: string;
    kind?: ObjectKindDto | undefined;
  },
  view: FilterView,
): string | null {
  const filters = linkableFilters(view);
  if (!filters) return null;
  return tableLink(profile, { ...table, kind: table.kind ?? 'table' }, filters);
}

/** Copy link text; WebKit needs the write inside the click. */
export function copyLink(link: string): Promise<void> {
  return navigator.clipboard.writeText(link);
}
