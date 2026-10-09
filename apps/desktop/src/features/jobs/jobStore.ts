// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';

/** Where a background job is. */
export type JobStatus = 'running' | 'finished' | 'failed' | 'cancelled';

/**
 * One background job of any kind (export, `.sql` file run, import…), as the
 * jobs popover lists it. Jobs live outside React, so closing the tab that
 * started one never stops it.
 */
export type Job = {
  id: string;
  /** What runs, e.g. `export` or `run-file`; shown as a small label. */
  kind: string;
  title: string;
  status: JobStatus;
  /** Progress or outcome in one line. */
  detail: string;
  /** Share done from 0 to 1; `undefined` while unknown. */
  fraction?: number | undefined;
  /** File or folder the job wrote, for Open and Show in Finder. */
  output?: string | undefined;
  startedAt: number;
  endedAt?: number | undefined;
  /** Stops the running job; the job reports `cancelled` when it ended. */
  cancel?: (() => void) | undefined;
  /** Shows the job's own view (a run's error list). */
  details?: (() => void) | undefined;
};

/** What a job's owner may change while it runs. */
export type JobPatch = Partial<
  Pick<Job, 'title' | 'detail' | 'fraction' | 'cancel' | 'details'>
>;

/** Controls of one added job. */
export type JobHandle = {
  id: string;
  update: (patch: JobPatch) => void;
  /** Done: `detail` describes the result; `output` is the written path. */
  finish: (detail: string, output?: string) => void;
  fail: (message: string) => void;
  cancelled: (detail?: string) => void;
};

let jobs: readonly Job[] = [];
let sequence = 0;
let popover = false;
const listeners = new Set<() => void>();

function emit(): void {
  listeners.forEach((listener) => {
    listener();
  });
}

function patch(id: string, change: Partial<Job>): void {
  jobs = jobs.map((job) => (job.id === id ? { ...job, ...change } : job));
  emit();
}

function end(id: string, change: Partial<Job>): void {
  patch(id, { ...change, endedAt: Date.now(), cancel: undefined });
}

/** Add a running job, newest first; its owner reports progress and the end. */
export function addJob(input: {
  kind: string;
  title: string;
  detail?: string;
  cancel?: () => void;
  details?: () => void;
}): JobHandle {
  sequence += 1;
  const id = `job-${String(sequence)}`;
  jobs = [
    {
      id,
      kind: input.kind,
      title: input.title,
      status: 'running',
      detail: input.detail ?? 'Starting…',
      startedAt: Date.now(),
      cancel: input.cancel,
      details: input.details,
    },
    ...jobs,
  ];
  emit();
  return {
    id,
    update: (change) => {
      patch(id, change);
    },
    finish: (detail, output) => {
      end(id, { status: 'finished', detail, output, fraction: 1 });
    },
    fail: (message) => {
      end(id, { status: 'failed', detail: message });
    },
    cancelled: (detail = 'Cancelled') => {
      end(id, { status: 'cancelled', detail });
    },
  };
}

/** Forget an ended job. */
export function removeJob(id: string): void {
  jobs = jobs.filter((job) => job.id !== id || job.status === 'running');
  emit();
}

/** Forget every ended job. */
export function clearEndedJobs(): void {
  jobs = jobs.filter((job) => job.status === 'running');
  emit();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Every job, newest first. */
export function useJobs(): readonly Job[] {
  return useSyncExternalStore(subscribe, () => jobs);
}

/** Open or close the jobs popover (the Show Background Jobs command). */
export function setJobsOpen(open: boolean): void {
  popover = open;
  emit();
}

/** Whether the jobs popover is open. */
export function useJobsOpen(): boolean {
  return useSyncExternalStore(subscribe, () => popover);
}
