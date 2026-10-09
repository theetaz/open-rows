// SPDX-License-Identifier: Apache-2.0
import { cancelImport } from '@openrows/ipc';
import type { ImportEventDto, ImportRequest } from '@openrows/ipc';
import { addJob } from '../jobs';
import type { JobHandle } from '../jobs';
import { rowCount } from './job';

/**
 * Show an import (not a dry run) in the background jobs popover too, with
 * its progress, Cancel and summary. Returns the event listener and a
 * setter for the import id once the core replied.
 */
export function mirrorImport(request: ImportRequest): {
  event: (event: ImportEventDto) => void;
  started: (importId: string) => void;
  rejected: (message: string) => void;
} | null {
  if (request.mode !== 'import') return null;
  const job: JobHandle = addJob({
    kind: 'import',
    title: `Import into ${request.target.table}`,
  });
  return {
    event: (event) => {
      reportImport(job, event);
    },
    started: (importId) => {
      job.update({
        cancel: () => {
          void cancelImport(importId);
        },
      });
    },
    rejected: (message) => {
      job.fail(message);
    },
  };
}

function reportImport(job: JobHandle, event: ImportEventDto): void {
  switch (event.type) {
    case 'progress':
      job.update({
        detail: `${rowCount(event.inserted)} written of ${rowCount(event.rows)} read`,
        fraction: event.fraction ?? undefined,
      });
      return;
    case 'finished':
      job.finish(
        `${rowCount(event.summary.inserted)} written, ${String(event.summary.failed ?? 0)} failed`,
      );
      return;
    case 'failed':
      job.fail(event.error.message);
      return;
    case 'cancelled':
      job.cancelled(
        `Cancelled; ${rowCount(event.summary.inserted)} committed earlier stayed`,
      );
  }
}
