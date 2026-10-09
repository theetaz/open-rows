// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  useSidebarPreferences,
  writeSidebarPreferences,
} from './sidebarPreferences';

/** Per-database schema choice of one connection. */
type Navigation = {
  schemas: Readonly<Record<string, string | undefined>>;
};

export type NavigationState = {
  chosenSchema: string | undefined;
  systemSchemas: boolean;
  setSchema: (schema: string) => void;
  setSystemSchemas: (show: boolean) => void;
};

const initial: Navigation = { schemas: {} };

/**
 * Schema choice per saved connection and database (the opened databases
 * themselves live with the connection's sessions). Survives reconnects and
 * switching connections or databases.
 */
export function useNavigation(
  profileId: string | null,
  database: string,
): NavigationState {
  const [navs, setNavs] = useState<
    Readonly<Record<string, Navigation | undefined>>
  >({});
  const preferences = useSidebarPreferences(profileId ?? '');
  const nav = (profileId && navs[profileId]) || initial;
  const update = (change: (current: Navigation) => Navigation): void => {
    if (!profileId) return;
    setNavs((current) => ({
      ...current,
      [profileId]: change(current[profileId] ?? initial),
    }));
  };
  return {
    chosenSchema: nav.schemas[database],
    systemSchemas: preferences.systemObjects,
    setSchema: (schema) => {
      update((current) => ({
        ...current,
        schemas: { ...current.schemas, [database]: schema },
      }));
    },
    setSystemSchemas: (show) => {
      if (profileId)
        writeSidebarPreferences(profileId, { systemObjects: show });
    },
  };
}
