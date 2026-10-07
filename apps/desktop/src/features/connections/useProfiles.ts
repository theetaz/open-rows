// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { listProfiles } from '@openrows/ipc';
import type { ProfileDto, ProfileList } from '@openrows/ipc';

type ProfilesState = {
  profiles: readonly ProfileDto[];
  /** Load failure or unreadable profile files, shown in the sidebar header. */
  problem: string | null;
  /** Read the list again after a change. */
  reload: () => void;
};

type Loaded = { profiles: readonly ProfileDto[]; problem: string | null };

function loaded(list: ProfileList): Loaded {
  return {
    profiles: list.profiles,
    problem: list.unreadable.length
      ? `Some connection files could not be read: ${list.unreadable.join('; ')}`
      : null,
  };
}

/** Saved connections as stored by the core; reloaded after every change. */
export function useProfiles(): ProfilesState {
  const [state, setState] = useState<Loaded>({ profiles: [], problem: null });
  const [generation, setGeneration] = useState(0);
  useEffect(() => {
    let active = true;
    listProfiles().then(
      (list) => {
        if (active) setState(loaded(list));
      },
      (error: unknown) => {
        const reason = error instanceof Error ? error.message : String(error);
        if (active)
          setState((current) => ({
            ...current,
            problem: `Could not load connections: ${reason}`,
          }));
      },
    );
    return () => {
      active = false;
    };
  }, [generation]);
  return {
    ...state,
    reload: () => {
      setGeneration((count) => count + 1);
    },
  };
}
