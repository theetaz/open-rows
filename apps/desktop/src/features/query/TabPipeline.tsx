// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';
import { isDocumentEngine } from '../connections/engines';
import { PipelineEditor } from './PipelineEditor';
import { isActive } from './run';
import type { RunState } from './run';
import type { QueryTab } from './tabs';
import type { QueryRuns } from './useQueryRuns';

/**
 * The aggregation pipeline editor of a MongoDB tab (M3-04). Stage previews run
 * through the tab's normal run path, so safety admission, cancellation and the
 * result pane are the editor's.
 */
export function TabPipeline(props: {
  tab: QueryTab;
  connection: ProfileDto;
  sessionId: string | null;
  runs: QueryRuns;
  run: RunState;
}): React.JSX.Element | null {
  const { sessionId } = props;
  if (!isDocumentEngine(props.connection.engine)) return null;
  return (
    <PipelineEditor
      disabled={!sessionId || isActive(props.run)}
      run={(sql) => {
        if (!sessionId) return;
        void props.runs.execute(props.tab.id, {
          sql,
          sessionId,
          mode: 'all',
          confirmToken: null,
          cursor: null,
          maxRows: 1000,
          returnOnError: true,
          params: null,
          autoLimit: null,
          confirmMultiple: false,
        });
      }}
    />
  );
}
