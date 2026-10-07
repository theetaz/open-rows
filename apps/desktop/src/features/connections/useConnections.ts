// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { deleteProfile, duplicateProfile } from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import { useProfiles } from './useProfiles';
import { useSessions } from './useSessions';
import type { SessionStatus } from './sessions';

/** Which modal sheet is open over the workspace. */
export type SheetState =
  | { kind: 'new' }
  | { kind: 'edit'; profile: ProfileDto }
  | { kind: 'delete'; profile: ProfileDto }
  | null;

export type Connections = {
  profiles: readonly ProfileDto[];
  problem: string | null;
  active: ProfileDto | undefined;
  statusOf: (profileId: string) => SessionStatus;
  select: (profileId: string) => void;
  selectIndex: (index: number) => void;
  connect: () => void;
  disconnect: () => void;
  sheet: SheetState;
  setSheet: (sheet: SheetState) => void;
  saved: (profile: ProfileDto) => void;
  duplicate: (profile: ProfileDto) => Promise<void>;
  remove: (profile: ProfileDto) => Promise<void>;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Saved connections, the active one, its session and the open sheet. */
export function useConnections(): Connections {
  const { profiles, problem, reload } = useProfiles();
  const sessions = useSessions();
  const [activeId, setActiveId] = useState<string | null>(null);
  const [sheet, setSheet] = useState<SheetState>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const active = profiles.find((profile) => profile.id === activeId);
  const select = (profileId: string): void => {
    setActiveId(profileId);
    setActionError(null);
    const state = sessions.statusOf(profileId).state;
    if (state === 'disconnected' || state === 'failed')
      void sessions.connect(profileId);
  };
  const run = async (action: () => Promise<void>): Promise<void> => {
    try {
      setActionError(null);
      await action();
    } catch (error) {
      setActionError(message(error));
    }
  };
  return {
    profiles,
    problem: actionError ?? problem,
    active,
    statusOf: sessions.statusOf,
    select,
    selectIndex: (index) => {
      const profile = profiles[index];
      if (profile) select(profile.id);
    },
    connect: () => {
      if (active) void sessions.connect(active.id);
    },
    disconnect: () => {
      if (active) void sessions.disconnect(active.id);
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
    remove: (profile) =>
      run(async () => {
        await sessions.disconnect(profile.id);
        await deleteProfile(profile.id);
        setSheet(null);
        if (activeId === profile.id) setActiveId(null);
        reload();
      }),
  };
}
