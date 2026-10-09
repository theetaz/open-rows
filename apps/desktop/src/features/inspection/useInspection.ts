// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import type { ObjectDetailDto } from '@openrows/ipc';
import { useResources } from '../schema/useResources';
import type { Resource } from '../schema/useResources';
import { readInspection } from './read';
import type { InspectionTarget } from './target';

/** Keyed catalog reads on open/refresh, with superseded replies kept isolated. */
export function useInspection(input: {
  profileId: string;
  database: string;
  target: InspectionTarget;
  connected: boolean;
  revision: number;
}): Resource<ObjectDetailDto> {
  const resources = useResources();
  const { profileId, database, target, connected, revision } = input;
  const key = JSON.stringify([profileId, database, target, revision]);
  const { load } = resources;
  useEffect(() => {
    if (!connected) return;
    load(key, () =>
      readInspection({ profileId, database, target }).then((data) => ({
        data,
        stale: false,
      })),
    );
  }, [load, key, profileId, database, target, connected]);
  return resources.get(key);
}
