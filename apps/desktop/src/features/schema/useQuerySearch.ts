// SPDX-License-Identifier: Apache-2.0
import { useMemo } from 'react';
import type { PaletteItem } from '@openrows/ui';
import { useSavedQueries } from '../query/savedQueries';
import { useProject } from '../projects';
import { absolutePath, stem } from '../projects/treeModel';
import { quickIndex } from './quickSearch';
/** Cache query descriptors; project SQL is read only when a result is opened. */
export function useQuerySearch(profileId: string): {
  index: ReturnType<typeof quickIndex>;
  saved: ReturnType<typeof useSavedQueries>;
  project: ReturnType<typeof useProject>['project'];
} {
  const saved = useSavedQueries(profileId || undefined);
  const { project } = useProject();
  const items = useMemo(
    (): PaletteItem[] => [
      ...saved.queries.map((query) => ({
        id: `saved:${query.id}`,
        label: query.name,
        detail: 'Local query',
        searchText: query.name,
      })),
      ...(project?.entries
        .filter((entry) => entry.kind === 'query')
        .map((entry) => ({
          id: `project:${absolutePath(project.root, entry.path)}`,
          label: stem(entry.path.split('/').at(-1) ?? entry.path),
          detail: `Project query · ${entry.path}`,
          searchText: `${stem(entry.path.split('/').at(-1) ?? entry.path)} ${entry.path}`,
        })) ?? []),
    ],
    [saved.queries, project],
  );
  const index = useMemo(() => quickIndex(items), [items]);
  return { index, saved, project };
}
