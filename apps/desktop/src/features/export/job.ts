// SPDX-License-Identifier: Apache-2.0
import type { ExportEventDto, ExportReply } from '@openrows/ipc';

/** One export as the sheet shows it. */
export type ExportJob =
  | { status: 'idle' }
  /** Waiting for the save panel and the core's reply. */
  | { status: 'starting' }
  | { status: 'running'; exportId: string; fileName: string; rows: number }
  | { status: 'finished'; fileName: string; path: string; rows: number }
  | { status: 'failed'; message: string }
  | { status: 'cancelled' };

export type JobAction =
  | { type: 'start' }
  | { type: 'reply'; reply: ExportReply }
  | { type: 'rejected'; message: string }
  | { type: 'event'; event: ExportEventDto };

export const idleJob: ExportJob = { status: 'idle' };

const fileNameOf = (path: string): string => path.split(/[\\/]/).pop() ?? path;

/**
 * Events may overtake the reply, so a `started` reply only fills in the id
 * of a job that is still starting or running.
 */
export function jobReducer(state: ExportJob, action: JobAction): ExportJob {
  switch (action.type) {
    case 'start':
      return { status: 'starting' };
    case 'rejected':
      return { status: 'failed', message: action.message };
    case 'reply':
      return applyReply(state, action.reply);
    case 'event':
      return applyEvent(state, action.event);
  }
}

function applyReply(state: ExportJob, reply: ExportReply): ExportJob {
  if (reply.status === 'dismissed') return idleJob;
  if (state.status === 'starting' || state.status === 'running')
    return {
      status: 'running',
      exportId: reply.exportId,
      fileName: reply.fileName,
      rows: state.status === 'running' ? state.rows : 0,
    };
  return state;
}

function applyEvent(state: ExportJob, event: ExportEventDto): ExportJob {
  switch (event.type) {
    case 'progress':
      if (state.status === 'running')
        return { ...state, rows: event.rows ?? state.rows };
      if (state.status === 'starting')
        return {
          status: 'running',
          exportId: '',
          fileName: '',
          rows: event.rows ?? 0,
        };
      return state;
    case 'finished':
      return {
        status: 'finished',
        path: event.path,
        fileName: fileNameOf(event.path),
        rows: event.rows ?? 0,
      };
    case 'failed':
      return { status: 'failed', message: event.error.message };
    case 'cancelled':
      return { status: 'cancelled' };
  }
}

/** True while the export can still be cancelled. */
export const isBusy = (job: ExportJob): boolean =>
  job.status === 'starting' || job.status === 'running';
