// SPDX-License-Identifier: Apache-2.0
import { cancelImport } from '@openrows/ipc';
import type { ImportEventDto, ImportRequest } from '@openrows/ipc';
import { addJob } from '../jobs';
import type { JobHandle } from '../jobs';
import { rowCount } from './job';
import { t } from './messages';

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
    title: t('importIntoTable2', { table: request.target.table }),
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
        detail: t('insertedWrittenRows', {
          inserted: rowCount(event.inserted),
          rows: rowCount(event.rows),
        }),
        fraction: event.fraction ?? undefined,
      });
      return;
    case 'finished':
      job.finish(
        t('insertedWrittenFailed', {
          inserted: rowCount(event.summary.inserted),
          failed: String(event.summary.failed ?? 0),
        }),
      );
      return;
    case 'failed':
      job.fail(event.error.message);
      return;
    case 'cancelled':
      job.cancelled(
        t('cancelledInsertedCommitted', {
          inserted: rowCount(event.summary.inserted),
        }),
      );
  }
}
