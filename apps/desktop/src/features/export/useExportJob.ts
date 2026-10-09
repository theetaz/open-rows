// SPDX-License-Identifier: Apache-2.0
import { useCallback, useReducer } from 'react';
import { cancelQuery, startExport } from '@openrows/ipc';
import type { ExportRequest } from '@openrows/ipc';
import { idleJob, jobReducer } from './job';
import type { ExportJob } from './job';

type ExportJobControls = {
  job: ExportJob;
  /** Show the save panel and start streaming; a cancelled panel returns to idle. */
  start: (request: ExportRequest) => void;
  /** Cancel a running export; its partial file is deleted by the core. */
  cancel: () => void;
};

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** One export's lifecycle: start, progress, cancel and the terminal notice. */
export function useExportJob(): ExportJobControls {
  const [job, dispatch] = useReducer(jobReducer, idleJob);
  const start = useCallback((request: ExportRequest): void => {
    dispatch({ type: 'start' });
    startExport(request, (event) => {
      dispatch({ type: 'event', event });
    }).then(
      (reply) => {
        dispatch({ type: 'reply', reply });
      },
      (error: unknown) => {
        dispatch({ type: 'rejected', message: messageOf(error) });
      },
    );
  }, []);
  const exportId = job.status === 'running' ? job.exportId : '';
  const cancel = useCallback((): void => {
    // The id arrives with the reply; progress may come first.
    if (exportId) void cancelQuery(exportId);
  }, [exportId]);
  return { job, start, cancel };
}
