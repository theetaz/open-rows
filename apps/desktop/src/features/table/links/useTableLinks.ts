// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { onCatalogChanged } from '@openrows/ipc';
import type { EngineDto } from '@openrows/ipc';
import type { TableTargetRef } from '../model';
import { noLinks } from './foreignKeys';
import type { TableLinks } from './foreignKeys';
import { invalidateLinks, readLinks } from './linkMetadata';

/** Foreign keys keyed by session and qualified table; stale replies cannot follow a switched table. */
export function useTableLinks(
  target: TableTargetRef,
  sessionId: string | null,
  engine: EngineDto,
): TableLinks {
  const key = JSON.stringify([target, sessionId, engine]);
  const [state, setState] = useState<{ key: string; links: TableLinks } | null>(
    null,
  );
  const { profileId, database, schema, table } = target;
  useEffect(() => {
    if (!sessionId) return;
    let live = true;
    let revision = 0;
    const load = (): void => {
      const current = ++revision;
      readLinks({ profileId, database, schema, table }, sessionId, engine).then(
        (links) => {
          if (live && revision === current) setState({ key, links });
        },
        () => {
          if (live && revision === current) setState({ key, links: noLinks });
        },
      );
    };
    load();
    const unsubscribe = onCatalogChanged((event) => {
      if (event.profileId !== profileId) return;
      invalidateLinks(profileId);
      load();
    });
    return () => {
      live = false;
      void unsubscribe.then((stop) => {
        stop();
      });
    };
  }, [profileId, database, schema, table, sessionId, engine, key]);
  return sessionId && state?.key === key ? state.links : noLinks;
}
