// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { schemaDiagram } from '@openrows/ipc';
import type { DiagramTarget, SchemaDiagramDto } from '@openrows/ipc';
import { layoutDiagram } from './layout';
import type { DiagramLayout } from './layout';

type DiagramState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; diagram: SchemaDiagramDto; layout: DiagramLayout };

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Read a schema's tables and keys once, then lay them out. */
export function useSchemaDiagram(target: DiagramTarget): DiagramState {
  const [state, setState] = useState<DiagramState>({ status: 'loading' });
  const { profileId, database, schema } = target;
  useEffect(() => {
    let live = true;
    schemaDiagram({ profileId, database, schema }).then(
      (diagram) => {
        if (live)
          setState({
            status: 'ready',
            diagram,
            layout: layoutDiagram(diagram),
          });
      },
      (error: unknown) => {
        if (live) setState({ status: 'error', message: messageOf(error) });
      },
    );
    return () => {
      live = false;
    };
  }, [profileId, database, schema]);
  return state;
}
