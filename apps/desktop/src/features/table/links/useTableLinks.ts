// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { schemaDiagram } from '@openrows/ipc';
import type { SchemaDiagramDto } from '@openrows/ipc';
import type { TableTargetRef } from '../model';
import { noLinks, tableLinks } from './foreignKeys';
import type { TableLinks } from './foreignKeys';

/** How long a schema's keys are reused before they are read again. */
const FRESH_MS = 60_000;

const cache = new Map<
  string,
  { at: number; read: Promise<SchemaDiagramDto> }
>();

/** The schema's ER data (tables and keys), shared by every tab of the schema. */
function keysOf(target: TableTargetRef): Promise<SchemaDiagramDto> {
  const key = JSON.stringify([
    target.profileId,
    target.database,
    target.schema,
  ]);
  const hit = cache.get(key);
  if (hit && Date.now() - hit.at < FRESH_MS) return hit.read;
  const read = schemaDiagram({
    profileId: target.profileId,
    database: target.database,
    schema: target.schema,
  });
  read.catch(() => {
    cache.delete(key);
  });
  cache.set(key, { at: Date.now(), read });
  return read;
}

/**
 * The browsed table's foreign keys from the ER diagram data (FR-GRID-07);
 * none until read, or when the session is closed or the read fails.
 */
export function useTableLinks(
  target: TableTargetRef,
  ready: boolean,
): TableLinks {
  const [links, setLinks] = useState<TableLinks>(noLinks);
  const { profileId, database, schema, table } = target;
  useEffect(() => {
    if (!ready) return;
    let live = true;
    keysOf({ profileId, database, schema, table }).then(
      (diagram) => {
        if (live) setLinks(tableLinks(diagram, schema, table));
      },
      () => {
        if (live) setLinks(noLinks);
      },
    );
    return () => {
      live = false;
    };
  }, [profileId, database, schema, table, ready]);
  return links;
}
