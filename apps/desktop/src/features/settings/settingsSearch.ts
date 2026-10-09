// SPDX-License-Identifier: Apache-2.0
import { createContext, useContext } from 'react';

/** The Settings search text; empty when not searching. */
export const SettingsQuery = createContext('');

/** Whether every word of `query` occurs in `texts` (case-insensitive). */
export function matchesQuery(query: string, texts: readonly string[]): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const haystack = texts.join(' ').toLowerCase();
  return words.every((word) => haystack.includes(word));
}

/** Whether a setting described by `texts` matches the current search. */
export function useSettingMatch(...texts: readonly string[]): boolean {
  return matchesQuery(useContext(SettingsQuery), texts);
}

/** Whether the Settings sheet is showing search results. */
export function useSearching(): boolean {
  return useContext(SettingsQuery).trim() !== '';
}
