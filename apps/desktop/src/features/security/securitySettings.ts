// SPDX-License-Identifier: Apache-2.0
import { localSetting } from '../settings/localSetting';
/** Validated security settings cache, mirrored in settings.json. */
type SecuritySettings = {
  lockOnStartup: boolean;
  lockAfterInactivityMinutes: number | null;
};
/** Security settings storage key. */
export const SECURITY_KEY = 'openrows.security';
/** Invalid lock options retain safe defaults. */
export function parseSecurity(value: unknown): SecuritySettings {
  const input =
    typeof value === 'object' && value !== null
      ? (value as Partial<SecuritySettings>)
      : {};
  return {
    lockOnStartup: input.lockOnStartup === true,
    lockAfterInactivityMinutes: [1, 5, 15, 30, 60].includes(
      input.lockAfterInactivityMinutes ?? 0,
    )
      ? (input.lockAfterInactivityMinutes ?? null)
      : null,
  };
}
/** Shared live security preferences. */
export const securitySettings = localSetting(SECURITY_KEY, parseSecurity);
