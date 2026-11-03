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
import { t, tn } from './messages';

/** Bytes as KB, MB or GB. */
export function sizeText(bytes: number): string {
  if (bytes < 1024) return t('sizeBytes', { bytes: String(bytes) });
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit] ?? 'TB'}`;
}

const errorCount = (errors: number): string =>
  tn('errorCount', errors, { count: errors.toLocaleString() });

function finishJob(
  job: JobHandle,
  event: Extract<RunFileEventDto, { type: 'finished' }>,
): void {
  const statements = event.statements ?? 0;
  const count = statements.toLocaleString();
  if (event.outcome === 'completed') {
    job.finish(tn('statementsRan', statements, { count }));
    return;
  }
  const ran = tn('statementCount', statements, { count });
  if (event.outcome === 'cancelled') {
    job.cancelled(
      event.rolledBack
        ? t('cancelledAfterRolledBack', { ran })
        : t('cancelledAfter', { ran }),
    );
    return;
  }
  const summary = event.errors
    ? t('ranWithErrors', { ran, errors: errorCount(event.errors) })
    : ran;
  job.fail(
    event.message ??
      (event.rolledBack ? t('summaryRolledBack', { summary }) : summary),
  );
}

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
    const done = tn('statementsOfTotal', run.statements, {
      count: run.statements.toLocaleString(),
      total: run.total.toLocaleString(),
    });
    job.update({
      detail: run.errorCount ? `${done} · ${errorCount(run.errorCount)}` : done,
      fraction: run.size > 0 ? run.bytes / run.size : undefined,
    });
    return;
  }
  if (event.type !== 'finished') return;
  finishJob(job, event);
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
    title: t('runName', { name: file.name }),
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
