// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { schemaDiagram } from '@openrows/ipc';
import type { DiagramTarget, SchemaDiagramDto } from '@openrows/ipc';
import { startLayout } from './layoutWorker';
import type { DiagramLayout } from './geometry';

type DiagramState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; diagram: SchemaDiagramDto; layout: DiagramLayout };

/** Read selected tables or the schema and lay them out off the main thread. */
export function useSchemaDiagram(target: DiagramTarget): DiagramState {
  const [state, setState] = useState<DiagramState & { key?: string }>({
    status: 'loading',
  });
  const { profileId, database, schema } = target;
  const tablesKey = JSON.stringify(target.tables ?? null);
  const key = JSON.stringify([profileId, database, schema, tablesKey]);
  useEffect(() => {
    const controller = new AbortController();
    const live = (): boolean => !controller.signal.aborted;
    let job: ReturnType<typeof startLayout> | undefined;
    const tables = JSON.parse(tablesKey) as string[] | null;
    const read = async (): Promise<void> => {
      const diagram = await schemaDiagram({
        profileId,
        database,
        schema,
        ...(tables ? { tables } : {}),
      });
      if (!live()) return;
      job = startLayout(diagram);
      const layout = await job.result;
      if (live()) setState({ key, status: 'ready', diagram, layout });
    };
    read().catch((error: unknown) => {
      if (live())
        setState({
          key,
          status: 'error',
          message: error instanceof Error ? error.message : String(error),
        });
    });
    return () => {
      controller.abort();
      job?.cancel();
    };
  }, [profileId, database, schema, tablesKey, key]);
  return state.key === key ? state : { status: 'loading' };
}
