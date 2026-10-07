// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';

/** Opened databases and per-database schema choice of one connection. */
type Navigation = {
  active: string;
  opened: readonly string[];
  schemas: Readonly<Record<string, string | undefined>>;
  systemSchemas: boolean;
};

export type NavigationState = {
  database: string;
  opened: readonly string[];
  chosenSchema: string | undefined;
  systemSchemas: boolean;
  openDatabase: (name: string) => void;
  closeDatabase: (name: string) => void;
  setSchema: (schema: string) => void;
  setSystemSchemas: (show: boolean) => void;
};

const initial = (database: string): Navigation => ({
  active: database,
  opened: [database],
  schemas: {},
  systemSchemas: false,
});

/** Close one database; the last opened database stays open. */
export function closeIn(nav: Navigation, name: string): Navigation {
  if (nav.opened.length < 2) return nav;
  const opened = nav.opened.filter((database) => database !== name);
  return {
    ...nav,
    opened,
    active: nav.active === name ? (opened[0] ?? nav.active) : nav.active,
  };
}

/**
 * Database navigation per saved connection; the session's own database is
 * opened first. Survives reconnects and switching connections.
 */
export function useNavigation(
  profileId: string | null,
  connectedDatabase: string,
): NavigationState {
  const [navs, setNavs] = useState<
    Readonly<Record<string, Navigation | undefined>>
  >({});
  const nav = (profileId && navs[profileId]) || initial(connectedDatabase);
  const update = (change: (current: Navigation) => Navigation): void => {
    if (!profileId) return;
    setNavs((current) => ({
      ...current,
      [profileId]: change(current[profileId] ?? initial(connectedDatabase)),
    }));
  };
  return {
    database: profileId ? nav.active : '',
    opened: profileId ? nav.opened : [],
    chosenSchema: nav.schemas[nav.active],
    systemSchemas: nav.systemSchemas,
    openDatabase: (name) => {
      update((current) => ({
        ...current,
        active: name,
        opened: current.opened.includes(name)
          ? current.opened
          : [...current.opened, name],
      }));
    },
    closeDatabase: (name) => {
      update((current) => closeIn(current, name));
    },
    setSchema: (schema) => {
      update((current) => ({
        ...current,
        schemas: { ...current.schemas, [current.active]: schema },
      }));
    },
    setSystemSchemas: (show) => {
      update((current) => ({ ...current, systemSchemas: show }));
    },
  };
}
