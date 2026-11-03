// SPDX-License-Identifier: Apache-2.0
import { cancelDump } from '@openrows/ipc';
import type { DumpEventDto } from '@openrows/ipc';
import { addJob, setJobsOpen } from '../jobs';
import type { JobHandle } from '../jobs';
import { sizeText } from '../runfile';
import { applyDumpEvent, getRun, putRun, showDump } from './dumpStore';
import type { DumpRun } from './dumpStore';
import { t, tn } from './messages';

type Starter = (
  onEvent: (event: DumpEventDto) => void,
) => Promise<{ jobId: string }>;

/** One line of progress: objects (of total) and bytes. */
export function progressText(run: DumpRun): string {
  const parts: string[] = [];
  if (run.objects > 0 || run.total !== null) {
    const objects = run.objects.toLocaleString();
    parts.push(
      run.total === null
        ? t('objectsDone', { objects })
        : t('objectsOfTotal', { objects, total: run.total.toLocaleString() }),
    );
  }
  if (run.bytes > 0) {
    const size = sizeText(run.bytes);
    parts.push(
      run.totalBytes
        ? t('sizeOfTotal', { size, total: sizeText(run.totalBytes) })
        : size,
    );
  }
  return parts.join(' · ') || t('starting');
}

/** Share done from 0 to 1, when a total is known. */
export function progressFraction(run: DumpRun): number | undefined {
  if (run.totalBytes) return Math.min(1, run.bytes / run.totalBytes);
  if (run.total) return Math.min(1, run.objects / run.total);
  return undefined;
}

/** What a finished job's row and log view say. */
export function outcomeText(run: DumpRun): string {
  const finished = run.finished;
  if (!finished) return '';
  if (finished.outcome === 'cancelled') return t('cancelled');
  if (finished.outcome === 'failed') {
    const message = finished.message ?? t('failed');
    return finished.exitCode === null
      ? message
      : t('failedWithExit', { message, exitCode: String(finished.exitCode) });
  }
  const what = outcomeHead(run);
  return run.bytes ? `${what} · ${sizeText(run.bytes)}` : what;
}

function outcomeHead(run: DumpRun): string {
  const backup = run.kind === 'backup';
  if (!run.objects) return backup ? t('backedUp') : t('restored');
  const count = run.objects.toLocaleString();
  return backup
    ? tn('backedUpObjects', run.objects, { count })
    : tn('restoredObjects', run.objects, { count });
}

function report(job: JobHandle, id: string, event: DumpEventDto): void {
  applyDumpEvent(id, event);
  const run = getRun(id);
  if (!run) return;
  if (event.type === 'progress') {
    job.update({ detail: progressText(run), fraction: progressFraction(run) });
    return;
  }
  if (event.type !== 'finished') return;
  if (event.outcome === 'completed')
    job.finish(outcomeText(run), event.output ?? undefined);
  else if (event.outcome === 'cancelled') job.cancelled(outcomeText(run));
  else job.fail(outcomeText(run));
}

/**
 * Start a backup or restore as a background job: it shows in the jobs
 * popover with progress and Cancel, and its log view opens.
 */
export function startDumpJob(input: {
  kind: DumpRun['kind'];
  title: string;
  start: Starter;
}): string {
  const job = addJob({
    kind: input.kind,
    title: input.title,
    details: () => {
      showDump({ kind: 'log', id: job.id });
    },
  });
  const id = job.id;
  putRun({
    id,
    kind: input.kind,
    title: input.title,
    startedAt: Date.now(),
    jobId: null,
    commands: [],
    log: [],
    objects: 0,
    total: null,
    bytes: 0,
    totalBytes: null,
    finished: null,
    failure: null,
  });
  input
    .start((event) => {
      report(job, id, event);
    })
    .then(
      ({ jobId }) => {
        const run = getRun(id);
        if (run) putRun({ ...run, jobId });
        job.update({
          cancel: () => {
            void cancelDump(jobId).catch(() => undefined);
          },
        });
      },
      (error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);
        const run = getRun(id);
        if (run) putRun({ ...run, failure: message });
        job.fail(message);
      },
    );
  setJobsOpen(true);
  return id;
}
