// SPDX-License-Identifier: Apache-2.0
import { cancelQuery, startExport } from '@openrows/ipc';
import type { ExportEventDto, ExportRequest } from '@openrows/ipc';
import { addJob, setJobsOpen } from '../jobs';
import type { JobHandle } from '../jobs';

export const rowCount = (rows: number): string =>
  `${rows.toLocaleString()} ${rows === 1 ? 'row' : 'rows'}`;

/** Show one export event on its job. */
function applyExportEvent(job: JobHandle, event: ExportEventDto): void {
  switch (event.type) {
    case 'progress': {
      const rows = `${rowCount(event.rows ?? 0)} written`;
      job.update({ detail: event.detail ? `${event.detail} · ${rows}` : rows });
      return;
    }
    case 'finished':
      job.finish(`${rowCount(event.rows ?? 0)} exported`, event.path);
      return;
    case 'failed':
      job.fail(event.error.message);
      return;
    case 'cancelled':
      job.cancelled('Cancelled. No file was written.');
  }
}

/**
 * Show the save (or folder) panel and start the export as a background
 * job in the jobs popover, which opens. Resolves `false` when the panel
 * was cancelled; rejects when the export could not start.
 */
export async function startExportJob(request: ExportRequest): Promise<boolean> {
  let job: JobHandle | null = null;
  // Events can overtake the reply; they wait for the job.
  const early: ExportEventDto[] = [];
  const reply = await startExport(request, (event) => {
    if (job) applyExportEvent(job, event);
    else early.push(event);
  });
  if (reply.status === 'dismissed') return false;
  const started = addJob({
    kind: 'export',
    title: `Export ${reply.fileName}`,
    cancel: () => {
      void cancelQuery(reply.exportId);
    },
  });
  job = started;
  early.forEach((event) => {
    applyExportEvent(started, event);
  });
  setJobsOpen(true);
  return true;
}
