// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { listObjects, tableStructure } from '@openrows/ipc';

/** Where the referencing table lives. */
type Scope = { profileId: string; database: string };

/** Most tables offered as referenced tables. */
const TABLE_LIMIT = 5000;

/**
 * The tables of `schema` and the columns of the chosen `table`; `onKey`
 * gets the chosen table's primary key once it is read.
 */
export function useReferenced(
  { profileId, database }: Scope,
  { schema, table }: { schema: string; table: string },
  onKey: (key: string[]) => void,
): { tables: readonly string[]; columns: readonly string[] } {
  const [tables, setTables] = useState<readonly string[]>([]);
  const [read, setRead] = useState<{
    table: string;
    columns: readonly string[];
  } | null>(null);
  const keyed = useRef(onKey);
  useEffect(() => {
    keyed.current = onKey;
  });
  useEffect(() => {
    let current = true;
    const section = { schema, kinds: ['table' as const], limit: TABLE_LIMIT };
    listObjects(profileId, database, section, false).then(
      (reply) => {
        if (current) setTables(reply.data.map((object) => object.name));
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [profileId, database, schema]);
  const name = table.trim();
  useEffect(() => {
    if (!name) return;
    let current = true;
    tableStructure({ profileId, database, schema, table: name }).then(
      (structure) => {
        if (!current) return;
        setRead({ table: name, columns: structure.columns.map((c) => c.name) });
        keyed.current(
          structure.columns.filter((c) => c.primaryKey).map((c) => c.name),
        );
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [profileId, database, schema, name]);
  return { tables, columns: read?.table === name ? read.columns : [] };
}
