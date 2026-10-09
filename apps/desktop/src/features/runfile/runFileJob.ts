// SPDX-License-Identifier: Apache-2.0
import { cancelQuery, pickRunFile, startRunFile } from '@openrows/ipc';
import type { PickedScriptDto, RunFileEventDto } from '@openrows/ipc';
import { addJob } from '../jobs';
import type { JobHandle } from '../jobs';
import {
  applyRunEvent,
  getRun,
  patchRun,
  putRun,
  showRunFile,
} from './runFileStore';
import type { RunTarget } from './runFileStore';

/** Bytes as KB, MB or GB. */
export function sizeText(bytes: number): string {
  if (bytes < 1024) return `${String(bytes)} bytes`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit] ?? 'TB'}`;
}

const count = (value: number, noun: string): string =>
  `${value.toLocaleString()} ${noun}${value === 1 ? '' : 's'}`;

/** File ▸ Run SQL File…: the open panel, then the sheet for the picked file. */
export async function pickRunFileFor(target: RunTarget): Promise<void> {
  const file = await pickRunFile();
  if (file) showRunFile({ kind: 'setup', file, target });
}

/** Progress and outcome of a run on its job. */
function report(job: JobHandle, id: string, event: RunFileEventDto): void {
  applyRunEvent(id, event);
  const run = getRun(id);
  if (!run) return;
  if (event.type === 'progress') {
    const errors = run.errorCount ? ` · ${count(run.errorCount, 'error')}` : '';
    job.update({
      detail: `${count(run.statements, 'statement')} of ${run.total.toLocaleString()}${errors}`,
      fraction: run.size > 0 ? run.bytes / run.size : undefined,
    });
    return;
  }
  if (event.type !== 'finished') return;
  const ran = count(event.statements ?? 0, 'statement');
  const errors = event.errors ? `, ${count(event.errors, 'error')}` : '';
  const rolledBack = event.rolledBack ? '; rolled back' : '';
  if (event.outcome === 'completed') job.finish(`${ran} ran`);
  else if (event.outcome === 'cancelled')
    job.cancelled(`Cancelled after ${ran}${rolledBack}`);
  else job.fail(event.message ?? `${ran}${errors}${rolledBack}`);
}

/** Start a checked file; the run shows in its sheet and the jobs popover. */
export function startFileRun(input: {
  file: PickedScriptDto;
  target: RunTarget;
  total: number;
  options: {
    stopOnError: boolean;
    transaction: boolean;
    confirmToken: string | null;
  };
}): string {
  const { file, target, options } = input;
  const job = addJob({
    kind: 'run-file',
    title: `Run ${file.name}`,
    details: () => {
      showRunFile({ kind: 'run', id: job.id });
    },
  });
  const id = job.id;
  putRun({
    id,
    file,
    target,
    size: file.bytes ?? 0,
    total: input.total,
    startedAt: Date.now(),
    runId: null,
    statements: 0,
    bytes: 0,
    errorCount: 0,
    current: '',
    errors: [],
    finished: null,
    failure: null,
  });
  startRunFile(
    {
      sessionId: target.sessionId,
      path: file.path,
      stopOnError: options.stopOnError,
      transaction: options.transaction,
      confirmToken: options.confirmToken,
    },
    (event) => {
      report(job, id, event);
    },
  ).then(
    (started) => {
      patchRun(id, { runId: started.runId });
      job.update({
        cancel: () => {
          void cancelQuery(started.runId);
        },
      });
    },
    (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      patchRun(id, { failure: message });
      job.fail(message);
    },
  );
  return id;
}
