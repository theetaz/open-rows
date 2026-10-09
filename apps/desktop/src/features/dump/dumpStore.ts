// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import type { DumpEventDto, PickedDumpDto } from '@openrows/ipc';

/** The open database a backup or restore acts on. */
export type DumpTarget = {
  /** An open session of the database; the job opens its own connections. */
  sessionId: string;
  profileId: string;
  /** Connection name, for titles. */
  connection: string;
  database: string;
  /** The server as the connection reported it, to pick matching tools. */
  serverProduct: string | null;
  serverVersion: string | null;
};

type Finished = Extract<DumpEventDto, { type: 'finished' }>;

/** A started backup or restore as its log view shows it. */
export type DumpRun = {
  id: string;
  kind: 'backup' | 'restore';
  title: string;
  startedAt: number;
  /** Id to cancel with, once the core replied. */
  jobId: string | null;
  /** The command of each step, as it started. */
  commands: readonly string[];
  /** The tool's stderr, the last {@link MAX_LOG_LINES} lines. */
  log: readonly string[];
  objects: number;
  total: number | null;
  bytes: number;
  totalBytes: number | null;
  finished: Finished | null;
  /** Why the job could not start. */
  failure: string | null;
};

/** What the dump sheet shows: a backup or restore to set up, or a job's log. */
type DumpView =
  | { kind: 'backup'; target: DumpTarget }
  | { kind: 'restore'; target: DumpTarget; file: PickedDumpDto }
  | { kind: 'log'; id: string };

/** Log lines kept per job; older lines are dropped. */
const MAX_LOG_LINES = 5_000;

let runs: ReadonlyMap<string, DumpRun> = new Map();
let view: DumpView | null = null;
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

/** Show a dump sheet, or close it with `null`. */
export function showDump(next: DumpView | null): void {
  view = next;
  emit();
}

export function useDumpView(): DumpView | null {
  return useSyncExternalStore(subscribe, () => view);
}

/** Record a run's state. */
export function putRun(run: DumpRun): void {
  const next = new Map(runs);
  next.set(run.id, run);
  runs = next;
  emit();
}

export function getRun(id: string): DumpRun | undefined {
  return runs.get(id);
}

export function useDumpRun(id: string): DumpRun | undefined {
  return useSyncExternalStore(subscribe, () => runs.get(id));
}

/** Apply one core event to a run. */
export function applyDumpEvent(id: string, event: DumpEventDto): void {
  const run = runs.get(id);
  if (!run) return;
  switch (event.type) {
    case 'step':
      putRun({ ...run, commands: [...run.commands, event.command] });
      return;
    case 'log': {
      const log = [...run.log, ...event.lines];
      putRun({ ...run, log: log.slice(-MAX_LOG_LINES) });
      return;
    }
    case 'progress':
      putRun({
        ...run,
        objects: event.objects ?? run.objects,
        total: event.total,
        bytes: event.bytes ?? run.bytes,
        totalBytes: event.totalBytes,
      });
      return;
    case 'finished':
      putRun({ ...run, finished: event });
  }
}
