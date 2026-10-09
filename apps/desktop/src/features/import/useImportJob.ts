// SPDX-License-Identifier: Apache-2.0
import { useCallback, useReducer } from 'react';
import { cancelImport, startImport } from '@openrows/ipc';
import type { ImportRequest } from '@openrows/ipc';
import { idleJob, importReducer } from './job';
import { mirrorImport } from './importJobMirror';
import type { ImportJob } from './job';

type ImportJobControls = {
  job: ImportJob;
  /** Start a dry run or import in the core. */
  start: (request: ImportRequest) => void;
  /** Cancel the running job; uncommitted rows are rolled back. */
  cancel: () => void;
  /** Back to the options after a finished dry run or a failure. */
  reset: () => void;
};

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** One import job's lifecycle: start, progress, cancel and the summary. */
export function useImportJob(): ImportJobControls {
  const [job, dispatch] = useReducer(importReducer, idleJob);
  const start = useCallback((request: ImportRequest): void => {
    dispatch({ type: 'start', mode: request.mode });
    const mirror = mirrorImport(request);
    startImport(request, (event) => {
      dispatch({ type: 'event', event });
      mirror?.event(event);
    }).then(
      (reply) => {
        dispatch({ type: 'started', importId: reply.importId });
        mirror?.started(reply.importId);
      },
      (error: unknown) => {
        dispatch({ type: 'rejected', message: messageOf(error) });
        mirror?.rejected(messageOf(error));
      },
    );
  }, []);
  const importId = job.status === 'running' ? job.importId : '';
  const cancel = useCallback((): void => {
    // The id arrives with the reply; progress may come first.
    if (importId) void cancelImport(importId);
  }, [importId]);
  const reset = useCallback((): void => {
    dispatch({ type: 'reset' });
  }, []);
  return { job, start, cancel, reset };
}
