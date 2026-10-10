// SPDX-License-Identifier: Apache-2.0
import { schemaDiagram, tableDetail } from '@openrows/ipc';
import type { EngineDto, SchemaDiagramDto } from '@openrows/ipc';
import type { TableTargetRef } from '../model';
import { detailLinks, tableLinks } from './foreignKeys';
import type { TableLinks } from './foreignKeys';

const FRESH_MS = 60_000;
type Cache<T> = Map<string, { at: number; read: Promise<T> }>;
const cache: Cache<TableLinks> = new Map();
const diagrams: Cache<SchemaDiagramDto> = new Map();

/** Table detail supplies outgoing schemas; the bounded ER read supplies same-schema incoming keys. */
export function readLinks(
  target: TableTargetRef,
  sessionId: string,
  engine: EngineDto,
): Promise<TableLinks> {
  const key = JSON.stringify([
    target.profileId,
    target.database,
    target.schema,
    target.table,
    sessionId,
    engine,
  ]);
  const diagramKey = JSON.stringify([
    target.profileId,
    target.database,
    target.schema,
    sessionId,
    engine,
  ]);
  return cached(cache, key, () =>
    Promise.all([
      tableDetail(target),
      cached(diagrams, diagramKey, () => schemaDiagram(target)),
    ]).then(([detail, diagram]) => {
      const links = detailLinks(
        detail,
        tableLinks(diagram, target.schema, target.table),
        target.schema,
      );
      // MySQL namespaces are databases; cross-database navigation is outside this feature.
      return engine === 'mysql' || engine === 'mariadb'
        ? {
            ...links,
            outgoing: links.outgoing.filter(
              (link) => link.schema === target.schema,
            ),
          }
        : links;
    }),
  );
}

/** Invalidate every table's keys when its connection's catalog changes. */
export function invalidateLinks(profileId: string): void {
  for (const entries of [cache, diagrams]) {
    for (const key of entries.keys()) {
      const parts: unknown = JSON.parse(key);
      if (Array.isArray(parts) && parts[0] === profileId) entries.delete(key);
    }
  }
}

function cached<T>(
  entries: Cache<T>,
  key: string,
  load: () => Promise<T>,
): Promise<T> {
  const hit = entries.get(key);
  if (hit && Date.now() - hit.at < FRESH_MS) return hit.read;
  const read = load();
  read.catch(() => {
    if (entries.get(key)?.read === read) entries.delete(key);
  });
  if (entries.size >= 200) {
    const oldest = entries.keys().next().value;
    if (oldest !== undefined) entries.delete(oldest);
  }
  entries.set(key, { at: Date.now(), read });
  return read;
}
