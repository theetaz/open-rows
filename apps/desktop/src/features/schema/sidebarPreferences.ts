// SPDX-License-Identifier: Apache-2.0
import { localSetting } from '../settings/localSetting';
import { objectGroups } from './catalog';

/** Versioned profile-scoped filter/visibility settings, mirrored to settings.json. */
export const SIDEBAR_KEY = 'openrows.sidebar.v1';
/** Filters affect catalog sections only; empty kind/schema lists mean no restriction. */
export type SidebarPreferences = {
  text: string;
  kinds: readonly string[];
  schemas: readonly string[];
  hidden: readonly string[];
  systemObjects: boolean;
};
const defaults: SidebarPreferences = {
  text: '',
  kinds: [],
  schemas: [],
  hidden: [],
  systemObjects: false,
};
type Profiles = Readonly<Record<string, SidebarPreferences>>;
const sectionIds = [
  'favourites',
  'recent',
  ...objectGroups.map((group) => group.id),
];
const strings = (value: unknown): string[] =>
  Array.isArray(value)
    ? [
        ...new Set(
          value.filter((item): item is string => typeof item === 'string'),
        ),
      ]
    : [];

function parsePreference(raw: unknown): SidebarPreferences {
  const value =
    raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  return {
    text: typeof value.text === 'string' ? value.text : '',
    kinds: strings(value.kinds).filter((id) =>
      objectGroups.some((group) => group.id === id),
    ),
    schemas: strings(value.schemas),
    hidden: strings(value.hidden).filter((id) => sectionIds.includes(id)),
    systemObjects: value.systemObjects === true,
  };
}

/** Validate each profile's preferences for both web storage and settings.json. */
export function parseSidebarPreferences(raw: unknown): Profiles {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {};
  return Object.fromEntries(
    Object.entries(raw).map(([id, value]) => [id, parsePreference(value)]),
  );
}
const preferences = localSetting(SIDEBAR_KEY, parseSidebarPreferences);

/** Current profile filters, with defaults for a new connection. */
export function readSidebarPreferences(profileId: string): SidebarPreferences {
  return preferences.get()[profileId] ?? defaults;
}

/** Change just this profile; the setting-file bridge persists the full map. */
export function writeSidebarPreferences(
  profileId: string,
  change: Partial<SidebarPreferences>,
): void {
  preferences.set({
    ...preferences.get(),
    [profileId]: { ...readSidebarPreferences(profileId), ...change },
  });
}

/** Live profile preferences restored across reconnects and databases. */
export function useSidebarPreferences(profileId: string): SidebarPreferences {
  return preferences.use()[profileId] ?? defaults;
}
