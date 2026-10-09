// SPDX-License-Identifier: Apache-2.0
import { cancelDump } from '@openrows/ipc';
import type { DumpEventDto } from '@openrows/ipc';
import { addJob, setJobsOpen } from '../jobs';
import type { JobHandle } from '../jobs';
import { sizeText } from '../runfile';
import { applyDumpEvent, getRun, putRun, showDump } from './dumpStore';
import type { DumpRun } from './dumpStore';

type Starter = (
  onEvent: (event: DumpEventDto) => void,
) => Promise<{ jobId: string }>;

const count = (value: number, noun: string): string =>
  `${value.toLocaleString()} ${noun}${value === 1 ? '' : 's'}`;

/** One line of progress: objects (of total) and bytes. */
export function progressText(run: DumpRun): string {
  const parts: string[] = [];
  if (run.objects > 0 || run.total !== null) {
    const of = run.total === null ? '' : ` of ${run.total.toLocaleString()}`;
    parts.push(`${run.objects.toLocaleString()}${of} objects`);
  }
  if (run.bytes > 0) {
    const of = run.totalBytes ? ` of ${sizeText(run.totalBytes)}` : '';
    parts.push(`${sizeText(run.bytes)}${of}`);
  }
  return parts.join(' · ') || 'Starting…';
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
  if (finished.outcome === 'cancelled') return 'Cancelled';
  if (finished.outcome === 'failed') {
    const code =
      finished.exitCode === null ? '' : ` (exit ${String(finished.exitCode)})`;
    return `${finished.message ?? 'Failed'}${code}`;
  }
  const what = run.kind === 'backup' ? 'Backed up' : 'Restored';
  const objects = run.objects ? ` ${count(run.objects, 'object')}` : '';
  const size = run.bytes ? ` · ${sizeText(run.bytes)}` : '';
  return `${what}${objects}${size}`;
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
