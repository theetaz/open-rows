// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import type { PickedScriptDto, RunFileEventDto } from '@openrows/ipc';

/** The open database a file runs against (on a session of its own). */
export type RunTarget = {
  /** An open session of the database; the run opens its own beside it. */
  sessionId: string;
  database: string;
  /** Connection name, for the sheet's title. */
  connection: string;
};

/** One failed statement. */
export type RunError = {
  statement: number;
  line: number;
  preview: string;
  message: string;
};

type Finished = Extract<RunFileEventDto, { type: 'finished' }>;

/** A started file run as its sheet shows it. */
export type FileRunState = {
  id: string;
  file: PickedScriptDto;
  target: RunTarget;
  /** File size in bytes. */
  size: number;
  /** Statements found by the check. */
  total: number;
  startedAt: number;
  /** Id to cancel with, once the core replied. */
  runId: string | null;
  statements: number;
  bytes: number;
  errorCount: number;
  current: string;
  errors: readonly RunError[];
  finished: Finished | null;
  /** Why the run could not start. */
  failure: string | null;
};

/** What the run-file sheet shows: a file to check and start, or a run. */
type RunFileView =
  | { kind: 'setup'; file: PickedScriptDto; target: RunTarget }
  | { kind: 'run'; id: string };

let runs: ReadonlyMap<string, FileRunState> = new Map();
let view: RunFileView | null = null;
const listeners = new Set<() => void>();

function emit(): void {
  listeners.forEach((listener) => {
    listener();
  });
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Show the sheet (or close it with `null`). */
export function showRunFile(next: RunFileView | null): void {
  view = next;
  emit();
}

export function useRunFileView(): RunFileView | null {
  return useSyncExternalStore(subscribe, () => view);
}

/** Record a run's state. */
export function putRun(run: FileRunState): void {
  const next = new Map(runs);
  next.set(run.id, run);
  runs = next;
  emit();
}

/** Change a recorded run. */
export function patchRun(id: string, change: Partial<FileRunState>): void {
  const run = runs.get(id);
  if (run) putRun({ ...run, ...change });
}

export function getRun(id: string): FileRunState | undefined {
  return runs.get(id);
}

export function useRun(id: string): FileRunState | undefined {
  return useSyncExternalStore(subscribe, () => runs.get(id));
}

/** Most errors kept for the list (the core reports at most this many). */
const MAX_ERRORS = 1_000;

/** Apply one core event to the run. */
export function applyRunEvent(id: string, event: RunFileEventDto): void {
  const run = runs.get(id);
  if (!run) return;
  switch (event.type) {
    case 'progress':
      putRun({
        ...run,
        statements: event.statements ?? run.statements,
        bytes: event.bytes ?? run.bytes,
        errorCount: event.errors ?? run.errorCount,
        current: event.current,
      });
      return;
    case 'error': {
      const error: RunError = {
        statement: event.statement ?? 0,
        line: event.line ?? 0,
        preview: event.preview,
        message: event.message,
      };
      const errors =
        run.errors.length < MAX_ERRORS ? [...run.errors, error] : run.errors;
      putRun({
        ...run,
        errors,
        errorCount: Math.max(run.errorCount, errors.length),
      });
      return;
    }
    case 'finished':
      putRun({
        ...run,
        finished: event,
        statements: event.statements ?? run.statements,
        errorCount: event.errors ?? run.errorCount,
        current: '',
      });
  }
}
