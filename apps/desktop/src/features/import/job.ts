// SPDX-License-Identifier: Apache-2.0
import type {
  ImportEventDto,
  ImportRequest,
  ImportSummaryDto,
} from '@openrows/ipc';

type Mode = ImportRequest['mode'];

/** Live counts of a running job. */
export type ImportProgress = {
  rows: number;
  inserted: number;
  failed: number;
  /** Share of the file read, 0–1. */
  fraction: number;
  rowsPerSecond: number;
};

/** One dry run or import as the sheet shows it. */
export type ImportJob =
  | { status: 'idle' }
  | { status: 'starting'; mode: Mode }
  | {
      status: 'running';
      mode: Mode;
      importId: string;
      progress: ImportProgress;
    }
  | { status: 'finished'; mode: Mode; summary: ImportSummaryDto }
  | {
      status: 'failed';
      mode: Mode;
      message: string;
      summary: ImportSummaryDto | null;
    }
  | { status: 'cancelled'; mode: Mode; summary: ImportSummaryDto | null };

type ImportAction =
  | { type: 'start'; mode: Mode }
  | { type: 'started'; importId: string }
  | { type: 'rejected'; message: string }
  | { type: 'event'; event: ImportEventDto }
  | { type: 'reset' };

export const idleJob: ImportJob = { status: 'idle' };

const noProgress: ImportProgress = {
  rows: 0,
  inserted: 0,
  failed: 0,
  fraction: 0,
  rowsPerSecond: 0,
};

const modeOf = (job: ImportJob): Mode =>
  job.status === 'idle' ? 'import' : job.mode;

/**
 * Events may overtake the reply, so `started` only fills in the id of a
 * job that is still starting or running.
 */
export function importReducer(
  state: ImportJob,
  action: ImportAction,
): ImportJob {
  switch (action.type) {
    case 'start':
      return { status: 'starting', mode: action.mode };
    case 'reset':
      return idleJob;
    case 'rejected':
      return {
        status: 'failed',
        mode: modeOf(state),
        message: action.message,
        summary: null,
      };
    case 'started':
      if (state.status === 'starting')
        return {
          status: 'running',
          mode: state.mode,
          importId: action.importId,
          progress: noProgress,
        };
      if (state.status === 'running')
        return { ...state, importId: action.importId };
      return state;
    case 'event':
      return applyEvent(state, action.event);
  }
}

function applyEvent(state: ImportJob, event: ImportEventDto): ImportJob {
  const mode = modeOf(state);
  switch (event.type) {
    case 'progress': {
      if (state.status !== 'starting' && state.status !== 'running')
        return state;
      const progress: ImportProgress = {
        rows: event.rows ?? 0,
        inserted: event.inserted ?? 0,
        failed: event.failed ?? 0,
        fraction: event.fraction ?? 0,
        rowsPerSecond: event.rowsPerSecond ?? 0,
      };
      const importId = state.status === 'running' ? state.importId : '';
      return { status: 'running', mode, importId, progress };
    }
    case 'finished':
      return { status: 'finished', mode, summary: event.summary };
    case 'failed':
      return {
        status: 'failed',
        mode,
        message: event.error.message,
        summary: event.summary,
      };
    case 'cancelled':
      return { status: 'cancelled', mode, summary: event.summary };
  }
}

/** True while the job can still be cancelled. */
export const isBusy = (job: ImportJob): boolean =>
  job.status === 'starting' || job.status === 'running';

/** `1 row`, `2,500 rows`. */
export const rowCount = (rows: number | null): string => {
  const value = rows ?? 0;
  return `${value.toLocaleString()} ${value === 1 ? 'row' : 'rows'}`;
};
