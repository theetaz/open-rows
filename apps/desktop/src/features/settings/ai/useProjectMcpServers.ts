// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import {
  discoverMcpTools,
  enableProjectMcpTools,
  listProjectMcpServers,
  promptProjectMcpSecret,
  revokeProjectMcpServer,
  trustProjectMcpServer,
} from '@openrows/ipc';
import type { ProjectServerView } from '@openrows/ipc';
import { mcpFailure } from './mcpFailure';
import { t } from '../messages/lazyAi';

const CHANGED = t('projectFileChanged');
const SECRET = {
  saved: t('savedKeychain'),
  cancelled: t('nothingEnteredStored'),
} as const;

/** A line under a server: an outcome, or a failure to announce. */
export type Note = { text: string; failed: boolean };
/** What the Project server section renders and does. */
export type ProjectServersControls = {
  servers: ProjectServerView[];
  busy: boolean;
  /** Failure or outcome, keyed by server key (`''` for the list). */
  notes: Partial<Record<string, Note>>;
  trust: (view: ProjectServerView) => Promise<void>;
  revoke: (view: ProjectServerView) => Promise<void>;
  discover: (view: ProjectServerView) => Promise<void>;
  toggle: (view: ProjectServerView, tool: string, on: boolean) => Promise<void>;
  secret: (view: ProjectServerView, name: string) => Promise<void>;
};

/** Servers of loaded Projects; every action reloads the native view. */
export function useProjectMcpServers(): ProjectServersControls {
  const [servers, setServers] = useState<ProjectServerView[]>([]);
  const [busy, setBusy] = useState(false);
  const [notes, setNotes] = useState<Partial<Record<string, Note>>>({});
  const note = useCallback((key: string, text: string, failed = false) => {
    setNotes((previous) => ({ ...previous, [key]: { text, failed } }));
  }, []);
  const reload = useCallback(async () => {
    setServers(await listProjectMcpServers());
  }, []);
  useEffect(() => {
    let live = true;
    listProjectMcpServers().then(
      (list) => {
        if (live) setServers(list);
      },
      (error: unknown) => {
        if (live) note('', mcpFailure(error, t('projectMcpServers')), true);
      },
    );
    return () => {
      live = false;
    };
  }, [note]);
  const act =
    <A extends unknown[]>(
      work: (view: ProjectServerView, ...args: A) => Promise<string>,
      failure: (error: unknown, view: ProjectServerView) => string,
    ) =>
    async (view: ProjectServerView, ...args: A): Promise<void> => {
      setBusy(true);
      note(view.key, '');
      try {
        note(view.key, await work(view, ...args));
      } catch (error) {
        note(view.key, failure(error, view), true);
      } finally {
        await reload().catch(() => undefined);
        setBusy(false);
      }
    };
  const generic = (error: unknown, view: ProjectServerView): string =>
    mcpFailure(error, view.server.name);
  return {
    servers,
    busy,
    notes,
    trust: act(
      async (view) => {
        await trustProjectMcpServer(view.key, view.digest);
        return '';
      },
      (error, view) =>
        kindOf(error) === 'invalid' ? CHANGED : generic(error, view),
    ),
    revoke: act(async (view) => {
      await revokeProjectMcpServer(view.key);
      return '';
    }, generic),
    discover: act(async (view) => {
      await discoverMcpTools(view.key);
      return '';
    }, generic),
    toggle: act(async (view, tool: string, on: boolean) => {
      const rest = view.enabledTools.filter((name) => name !== tool);
      await enableProjectMcpTools(view.key, on ? [...rest, tool] : rest);
      return '';
    }, generic),
    secret: act(async (view, name: string) => {
      return `${name}: ${SECRET[await promptProjectMcpSecret(view.key, name)]}`;
    }, generic),
  };
}

function kindOf(error: unknown): string {
  return error && typeof error === 'object' && 'kind' in error
    ? String(error.kind)
    : '';
}
