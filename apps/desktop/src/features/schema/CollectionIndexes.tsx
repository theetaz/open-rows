// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { TreeNote } from '@openrows/ui';
import { tableStructure } from '@openrows/ipc';
import type { TableIndexDto } from '@openrows/ipc';
type TableTarget = Parameters<typeof tableStructure>[0];

/** Read indexes only while a MongoDB collection is expanded in the schema tree. */
export function CollectionIndexes({
  target,
}: {
  target: TableTarget;
}): React.JSX.Element {
  const [indexes, setIndexes] = useState<TableIndexDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { profileId, database, schema, table } = target;
  useEffect(() => {
    let live = true;
    tableStructure({ profileId, database, schema, table }).then(
      (value) => {
        if (live) setIndexes(value.indexes);
      },
      (failure: unknown) => {
        if (live)
          setError(
            failure instanceof Error ? failure.message : String(failure),
          );
      },
    );
    return () => {
      live = false;
    };
  }, [profileId, database, schema, table]);
  if (error)
    return (
      <TreeNote level={2} tone="danger">
        {error}
      </TreeNote>
    );
  if (!indexes) return <TreeNote level={2}>Loading indexes…</TreeNote>;
  return (
    <>
      <TreeNote level={2}>Indexes</TreeNote>
      {indexes.map((index) => (
        <TreeNote key={index.name} level={3}>
          {index.name} · {index.columns}
          {index.unique ? ' · unique' : ''}
        </TreeNote>
      ))}
    </>
  );
}
