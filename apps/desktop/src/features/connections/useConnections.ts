// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  confirmDestructive,
  deleteProfile,
  duplicateProfile,
} from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import { useProfiles } from './useProfiles';
import { useSessions } from './useSessions';
import { useDatabases } from './useDatabases';
import { useSessionRestore } from './useSessionRestore';
import { usePasswordPrompt } from './usePasswordPrompt';
import type { PasswordRequest } from './usePasswordPrompt';
import { railOrder } from './workspaceGroups';
import type { Databases } from './useDatabases';
import type { SessionStatus } from './sessions';

/** Which modal sheet is open over the workspace. */
export type SheetState =
  | { kind: 'new' }
  | { kind: 'new-file'; path: string }
  | { kind: 'import' }
  | { kind: 'export'; ids?: string[] }
  | { kind: 'edit'; profile: ProfileDto }
  | { kind: 'delete'; profile: ProfileDto }
  | null;

export type Connections = Databases & {
  /** Database descriptors visible before a connection is activated. */
  restoredDatabases?: (id: string) => readonly string[];
  profiles: readonly ProfileDto[];
  reload: () => void;
  problem: string | null;
  active: ProfileDto | undefined;
  statusOf: (profileId: string) => SessionStatus;
  select: (profileId: string, database?: string) => void;
  selectIndex: (index: number) => void;
  connect: () => void;
  disconnect: () => void;
  /** Disconnect any saved profile, active or not. */
  disconnectProfile: (profileId: string) => void;
  sheet: SheetState;
  setSheet: (sheet: SheetState) => void;
  saved: (profile: ProfileDto) => void;
  duplicate: (profile: ProfileDto) => Promise<void>;
  remove: (profile: ProfileDto) => Promise<void>;
  /** Native alert first; the in-app alert sheet outside the native shell. */
  confirmRemove: (profile: ProfileDto) => void;
  /** The open password sheet of an "ask every time" connect. */
  passwordRequest: PasswordRequest | null;
};

/** The macOS alert text for deleting a connection. */
export function deletePrompt(profile: ProfileDto): {
  title: string;
  message: string;
  action: string;
} {
  return {
    title: `Delete “${profile.name}”?`,
    message:
      'Its saved password is removed from the Keychain. Databases are not touched.',
    action: 'Delete',
  };
}

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

function askToRemove(
  profile: ProfileDto,
  remove: (profile: ProfileDto) => Promise<void>,
  setSheet: (sheet: SheetState) => void,
): void {
  const fallback = (): void => {
    setSheet({ kind: 'delete', profile });
  };
  confirmDestructive(deletePrompt(profile)).then((answer) => {
    if (answer === null) fallback();
    else if (answer) void remove(profile);
  }, fallback);
}

/** Saved connections, the active one, its session and the open sheet. */
export function useConnections(): Connections {
  const { profiles, problem, reload, loaded } = useProfiles();
  const prompt = usePasswordPrompt(profiles);
  const sessions = useSessions(prompt.ask);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [sheet, setSheet] = useState<SheetState>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const active = profiles.find((profile) => profile.id === activeId);
  const databases = useDatabases(sessions, active?.id);
  const restored = useSessionRestore(
    { profiles, activeId, setActiveId, sessions, databases },
    loaded,
  );
  const select = (profileId: string, database?: string): void => {
    setActiveId(profileId);
    setActionError(null);
    const state = sessions.statusOf(profileId).state;
    if (database) databases.prefer(profileId, database);
    if (state === 'disconnected' || state === 'failed')
      connectWanted(sessions, databases, profileId, database);
    else if (database && state === 'connected')
      databases.chooseDatabase(profileId, database);
  };
  const run = async (action: () => Promise<void>): Promise<void> => {
    try {
      setActionError(null);
      await action();
    } catch (error) {
      setActionError(message(error));
    }
  };
  const remove = (profile: ProfileDto): Promise<void> =>
    run(async () => {
      await sessions.disconnect(profile.id);
      await deleteProfile(profile.id);
      setSheet(null);
      if (activeId === profile.id) setActiveId(null);
      reload();
    });
  return {
    ...databases,
    restoredDatabases: restored.databasesOf,
    ...restoredDatabaseActions(restored, { databases, sessions, active }),
    profiles,
    reload,
    problem: actionError ?? problem,
    active,
    statusOf: sessions.statusOf,
    select,
    selectIndex: (index) => {
      const profile = railOrder(profiles)[index];
      if (profile) select(profile.id);
    },
    connect: () => {
      if (active) connectWanted(sessions, databases, active.id);
    },
    sheet,
    setSheet,
    saved: (profile) => {
      setSheet(null);
      reload();
      setActiveId(profile.id);
    },
    duplicate: (profile) =>
      run(async () => {
        const copy = await duplicateProfile(profile.id);
        setSheet(null);
        reload();
        setActiveId(copy.id);
      }),
    remove,
    confirmRemove: (profile) => {
      askToRemove(profile, remove, setSheet);
    },
    passwordRequest: prompt.request,
  };
}

/** Closing a database also removes its disconnected descriptor. */
function restoredDatabaseActions(
  restored: ReturnType<typeof useSessionRestore>,
  input: {
    databases: Databases;
    sessions: ReturnType<typeof useSessions>;
    active: ProfileDto | undefined;
  },
): Pick<
  Connections,
  'closeDatabase' | 'closeProfileDatabase' | 'disconnect' | 'disconnectProfile'
> {
  const { databases, sessions, active } = input;
  const disconnectProfile = (id: string): void => {
    restored.rememberConnection(id);
    void sessions.disconnect(id);
  };
  return {
    disconnectProfile,
    disconnect: () => {
      if (active) disconnectProfile(active.id);
    },
    closeProfileDatabase: (id, name) => {
      restored.forgetDatabase(id, name);
      databases.closeProfileDatabase(id, name);
    },
    closeDatabase: (name) => {
      if (active) restored.forgetDatabase(active.id, name);
      databases.closeDatabase(name);
    },
  };
}

/** An explicit connection selection opens only its remembered active database. */
function connectWanted(
  sessions: ReturnType<typeof useSessions>,
  databases: Databases,
  id: string,
  database?: string,
): void {
  const wanted = database ?? databases.databaseOf(id);
  void sessions.connect(id).then((home) => {
    if (home && wanted && wanted !== home)
      void sessions.openDatabase(id, wanted);
  });
}
