// SPDX-License-Identifier: Apache-2.0
import { createRoot } from 'react-dom/client';
import { ThemeProvider } from '@openrows/ui';
import {
  createProfile,
  onMonitorChanged,
  openConnection,
  openSession,
} from '@openrows/ipc';
import type { ProfileDto, ProfileInput } from '@openrows/ipc';
import type { SpikeSession } from './lifecycle';
import { withTimeout } from './lifecycle';

/** `just bench-memory --scenario monitor` modes (M25-09). */
export type MemoryMode = 'connected' | 'monitor';

/**
 * A disposable in-memory SQLite connection, so neither mode needs a server or
 * touches user data: the benchmark runs the app with an isolated home folder.
 */
function memoryProfile(): ProfileInput {
  return {
    name: 'Memory benchmark',
    group: null,
    env: { kind: 'local', label: null, color: null },
    engine: 'sqlite',
    file: { path: '', memory: true, readOnly: false, create: false },
    host: '',
    port: 0,
    database: null,
    user: null,
    tlsMode: 'disable',
    safety: 'unrestricted',
    passwordStorage: 'none',
    password: { kind: 'clear' },
  };
}

/**
 * Connect, and in `monitor` mode open the Monitor workspace on the session
 * until its collector has published a sample; then report readiness and stay
 * open until the benchmark stops the app (the footprint is sampled meanwhile).
 */
export async function runMemorySpike(
  session: SpikeSession,
  mode: MemoryMode,
): Promise<void> {
  const profile = await createProfile(memoryProfile());
  await openConnection(profile.id);
  const opened = await openSession(profile.id, null);
  session.progress('memory-connected', opened.server.database);
  if (mode === 'monitor')
    await withTimeout(
      openMonitor(profile, opened.server.database, opened.sessionId),
      20_000,
      'Monitor first sample',
    );
  // The same overlay without the Monitor, so the increment excludes its layer.
  else mountOverlay(<p role="status">Connected</p>);
  session.progress('memory-ready', mode);
}

/** A full-window layer over the workspace, like a tab's content in front of it. */
function mountOverlay(content: React.JSX.Element): void {
  const host = document.createElement('div');
  host.style.cssText =
    'position:fixed;inset:0;z-index:999;overflow:auto;background:var(--or-window-bg, Canvas);';
  document.body.append(host);
  createRoot(host).render(<ThemeProvider>{content}</ThemeProvider>);
}

/** Mount the lazy Monitor tab as the workspace does and await one collector sample. */
async function openMonitor(
  profile: ProfileDto,
  database: string,
  sessionId: string,
): Promise<void> {
  let sampled: () => void = () => undefined;
  const firstSample = new Promise<void>((resolve) => {
    sampled = resolve;
  });
  const unlisten = await onMonitorChanged((event) => {
    if (event.target.profileId === profile.id) sampled();
  });
  const { MonitorTab } = await import('../features/monitor/MonitorTab');
  mountOverlay(
    <MonitorTab
      profile={profile}
      database={database}
      connected
      sessionId={sessionId}
      onOpenSql={() => undefined}
    />,
  );
  try {
    await firstSample;
  } finally {
    unlisten();
  }
}
