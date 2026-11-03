// SPDX-License-Identifier: Apache-2.0
import type { OpenLinkDto, ProfileDto } from '@openrows/ipc';
import { isDocumentEngine } from '../connections/engines';
import { operators } from '../table/model';
import { t } from './messages';

/** A link and the saved connection it names. */
export type ResolvedLink = { link: OpenLinkDto; profile: ProfileDto };

/**
 * Find the saved connection a link names: its id first, else a unique name.
 * Returns a message when no single connection matches.
 */
export function resolveLink(
  link: OpenLinkDto,
  profiles: readonly ProfileDto[],
): ResolvedLink | { error: string } {
  const { id, name } = link.connection;
  const byId = id ? profiles.find((profile) => profile.id === id) : undefined;
  if (byId) return { link, profile: byId };
  const named = name ? profiles.filter((profile) => profile.name === name) : [];
  if (named.length === 1 && named[0]) return { link, profile: named[0] };
  if (named.length > 1)
    return {
      error: t('severalSavedConnections', { name: name ?? '' }),
    };
  return {
    error: t('noSavedConnection'),
  };
}

/** Where a profile connects, without any credential. */
function serverText(profile: ProfileDto): string {
  if (profile.file) return profile.file.path;
  return `${profile.engine} · ${profile.host}:${String(profile.port)}`;
}

function operatorText(operator: string): string {
  return operators.find((item) => item.value === operator)?.label ?? operator;
}

/** The preview rows of the confirmation sheet, in reading order. */
export function linkRows(resolved: ResolvedLink): [string, string][] {
  const { link, profile } = resolved;
  const environment = profile.env.label ?? profile.env.kind;
  const rows: [string, string][] = [
    [t('connection'), `${profile.name} (${environment})`],
    [t('server'), serverText(profile)],
  ];
  if (link.kind === 'connection') {
    if (link.database) rows.push([t('database'), link.database]);
    return rows;
  }
  // A MongoDB database is its only schema; its tables are collections.
  const documents = isDocumentEngine(profile.engine);
  rows.push([t('database'), link.database]);
  if (!documents) rows.push([t('schema'), link.schema]);
  if (link.object) {
    const kind =
      documents && link.object.kind === 'table'
        ? 'collection'
        : link.object.kind;
    rows.push([t('opens'), `${link.object.name} (${kind})`]);
  }
  if (link.kind === 'table')
    for (const filter of link.filters)
      rows.push([
        t('filter'),
        `${filter.column} ${operatorText(filter.operator)} ${filter.value}${filter.second ? ` … ${filter.second}` : ''}`.trim(),
      ]);
  return rows;
}
