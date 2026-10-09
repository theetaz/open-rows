// SPDX-License-Identifier: Apache-2.0
import { useEffect, useSyncExternalStore } from 'react';
import { chooseProject, loadProject } from '@openrows/ipc';
import type { ProjectDto } from '@openrows/ipc';

/** Recent projects kept for the Queries sidebar's project menu. */
const MAX_RECENT = 8;
/** Web storage key of the open project's root and the recent roots. */
const STORAGE_KEY = 'openrows.projects';

type Stored = { current: string | null; recent: readonly string[] };

/** The open project, the recent roots and the last failure. */
type ProjectState = {
  project: ProjectDto | null;
  recent: readonly string[];
  /** The project is being read. */
  loading: boolean;
  error: string | null;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Read the stored roots; invalid storage reads as none. */
export function parseStored(text: string | null): Stored {
  try {
    const value = text ? (JSON.parse(text) as Partial<Stored>) : {};
    const recent = Array.isArray(value.recent)
      ? value.recent.filter((root): root is string => typeof root === 'string')
      : [];
    const current = typeof value.current === 'string' ? value.current : null;
    return { current, recent: recent.slice(0, MAX_RECENT) };
  } catch {
    return { current: null, recent: [] };
  }
}

function readStored(): Stored {
  try {
    return parseStored(localStorage.getItem(STORAGE_KEY));
  } catch {
    return { current: null, recent: [] };
  }
}

function writeStored(stored: Stored): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(stored));
  } catch {
    /* Without web storage the project is forgotten on relaunch. */
  }
}

let state: ProjectState = {
  project: null,
  recent: readStored().recent,
  loading: false,
  error: null,
};
let restored = false;
const listeners = new Set<() => void>();

function set(change: Partial<ProjectState>): void {
  state = { ...state, ...change };
  writeStored({ current: state.project?.root ?? null, recent: state.recent });
  listeners.forEach((listener) => {
    listener();
  });
}

/** `root` first, then the other recent roots. */
export function withRecent(
  recent: readonly string[],
  root: string,
): readonly string[] {
  return [root, ...recent.filter((item) => item !== root)].slice(0, MAX_RECENT);
}

function opened(project: ProjectDto): void {
  set({
    project,
    recent: withRecent(state.recent, project.root),
    loading: false,
    error: null,
  });
}

function failed(error: unknown): void {
  set({ loading: false, error: message(error) });
}

/** File ▸ Open Project…: pick a folder (it becomes a project if needed). */
export function openProjectDialog(): void {
  chooseProject().then((project) => {
    if (project) opened(project);
  }, failed);
}

/** Open a recent project by its root. */
export function openProject(root: string): void {
  set({ loading: true, error: null });
  loadProject(root).then(opened, failed);
}

/** File ▸ Close Project: the sidebar shows local queries only. */
export function closeProject(): void {
  set({ project: null, error: null });
}

/** Read the open project again (after a change in the tree or on disk). */
export function reloadProject(): Promise<void> {
  const root = state.project?.root;
  if (!root) return Promise.resolve();
  return loadProject(root).then(opened, failed);
}

/** Replace the open project with a newer reading of it. */
export function showProject(project: ProjectDto): void {
  opened(project);
}

/** Report a failed project action in the sidebar. */
export function reportProjectError(error: unknown): void {
  failed(error);
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Reopen the project of the last session, once. */
function restore(): void {
  if (restored) return;
  restored = true;
  const { current } = readStored();
  if (current) openProject(current);
}

/** The open project, shared live; reopens the last one on first use. */
export function useProject(): ProjectState {
  useEffect(restore, []);
  return useSyncExternalStore(subscribe, () => state);
}
