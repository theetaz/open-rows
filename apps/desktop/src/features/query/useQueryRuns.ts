// SPDX-License-Identifier: Apache-2.0
import { useReducer } from 'react';
import {
  CommandError,
  cancelQuery,
  executeQuery,
  releaseResults,
} from '@openrows/ipc';
import type { ExecuteRequest } from '@openrows/ipc';
import { idleRun, runReducer } from './run';
import type { RunAction, RunState } from './run';
import { announceHistoryChange } from './historyEvents';

type Runs = Readonly<Record<string, RunState | undefined>>;

function runsReducer(
  runs: Runs,
  { tabId, action }: { tabId: string; action: RunAction | null },
): Runs {
  if (!action)
    return Object.fromEntries(
      Object.entries(runs).filter(([id]) => id !== tabId),
    );
  return { ...runs, [tabId]: runReducer(runs[tabId] ?? idleRun, action) };
}

export type QueryRuns = {
  runOf: (tabId: string) => RunState;
  execute: (tabId: string, request: ExecuteRequest) => Promise<void>;
  confirm: (tabId: string) => void;
  dismiss: (tabId: string) => void;
  cancel: (tabId: string) => void;
  /** Forget a closed tab's run and free its results in the core. */
  release: (tabId: string) => void;
};

/** Free result memory; a failure only means the core already dropped them. */
function free(run: RunState | undefined): void {
  const ids = run?.results.map((result) => result.id) ?? [];
  releaseResults(ids).catch(() => undefined);
}

function rejection(error: unknown): RunAction {
  if (error instanceof CommandError)
    return { type: 'rejected', message: error.message, code: error.code };
  return {
    type: 'rejected',
    message: error instanceof Error ? error.message : String(error),
    code: null,
  };
}

/** Runs of every query tab; events update result metadata, never rows. */
export function useQueryRuns(): QueryRuns {
  const [runs, dispatch] = useReducer(runsReducer, {});
  const execute = async (
    tabId: string,
    request: ExecuteRequest,
  ): Promise<void> => {
    free(runs[tabId]);
    dispatch({ tabId, action: { type: 'begin' } });
    try {
      const reply = await executeQuery(request, (event) => {
        dispatch({ tabId, action: { type: 'event', event } });
        // The core records history before it sends `finished`, so lists refetch now.
        if (event.type === 'finished') announceHistoryChange();
      });
      dispatch({ tabId, action: { type: 'reply', reply, request } });
    } catch (error) {
      dispatch({ tabId, action: rejection(error) });
    }
  };
  return {
    runOf: (tabId) => runs[tabId] ?? idleRun,
    execute,
    confirm: (tabId) => {
      const pending = runs[tabId]?.pending;
      if (pending)
        void execute(tabId, {
          ...pending.request,
          confirmToken: pending.token,
        });
    },
    dismiss: (tabId) => {
      dispatch({ tabId, action: { type: 'dismiss' } });
    },
    cancel: (tabId) => {
      const queryId = runs[tabId]?.queryId;
      if (queryId) cancelQuery(queryId).catch(() => undefined);
    },
    release: (tabId) => {
      free(runs[tabId]);
      dispatch({ tabId, action: null });
    },
  };
}
