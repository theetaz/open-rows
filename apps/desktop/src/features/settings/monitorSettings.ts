// SPDX-License-Identifier: Apache-2.0
import { localSetting } from './localSetting';
/** Device-local history opt-ins, excluded from portable preferences. */
export const MONITOR_KEY = 'openrows.monitoring';
/** Only UUID connection identities can opt in; missing preferences default off. */
export function parseMonitorSettings(value: unknown): {
  enabledProfiles: string[];
} {
  const profiles = (value as { enabledProfiles?: unknown } | null)
    ?.enabledProfiles;
  return {
    enabledProfiles: Array.isArray(profiles)
      ? [
          ...new Set(
            profiles
              .filter(
                (id): id is string =>
                  typeof id === 'string' && /^[a-fA-F0-9-]{36}$/.test(id),
              )
              .map((id) => id.toLowerCase()),
          ),
        ].slice(0, 1000)
      : [],
  };
}
/** Shared explicit history settings; enabling never starts a collector. */
export const monitorSettings = localSetting(MONITOR_KEY, parseMonitorSettings);
