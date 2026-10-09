// SPDX-License-Identifier: Apache-2.0
import { localSetting } from './localSetting';
/** Count/age retention for history, mirrored in settings.json. */
type HistorySettings = {
  maxEntries: number;
  maxAgeDays: number;
  keepForever: boolean;
};
/** Synchronous preference cache key. */
export const HISTORY_KEY = 'openrows.history';
const defaults: HistorySettings = {
  maxEntries: 100_000,
  maxAgeDays: 90,
  keepForever: false,
};
/** Validate bounds identically to the Rust retention policy. */
export function parseHistorySettings(stored: unknown): HistorySettings {
  const value =
    typeof stored === 'object' && stored !== null
      ? (stored as Record<string, unknown>)
      : {};
  const count = (key: 'maxEntries' | 'maxAgeDays'): number => {
    const item = value[key];
    return typeof item === 'number' &&
      Number.isInteger(item) &&
      item > 0 &&
      item <= 4_294_967_295
      ? item
      : defaults[key];
  };
  return {
    maxEntries: count('maxEntries'),
    maxAgeDays: count('maxAgeDays'),
    keepForever: value.keepForever === true,
  };
}
/** Shared live history preferences. */
export const historySettings = localSetting(HISTORY_KEY, parseHistorySettings);
