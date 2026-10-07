// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';

/** Per-database schema choice of one connection. */
type Navigation = {
  schemas: Readonly<Record<string, string | undefined>>;
  systemSchemas: boolean;
};

export type NavigationState = {
  chosenSchema: string | undefined;
  systemSchemas: boolean;
  setSchema: (schema: string) => void;
  setSystemSchemas: (show: boolean) => void;
};

const initial: Navigation = { schemas: {}, systemSchemas: false };

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
    systemSchemas: nav.systemSchemas,
    setSchema: (schema) => {
      update((current) => ({
        ...current,
        schemas: { ...current.schemas, [database]: schema },
      }));
    },
    setSystemSchemas: (show) => {
      update((current) => ({ ...current, systemSchemas: show }));
    },
  };
}
