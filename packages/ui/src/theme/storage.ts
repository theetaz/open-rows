// SPDX-License-Identifier: Apache-2.0
/** Persisted UI-only theme preference. */
export type ThemeMode = 'system' | 'light' | 'dark';
/** Validate persisted preferences rather than trusting local storage. */
export function readTheme(): ThemeMode {
  try {
    const mode = localStorage.getItem('openrows.theme');
    return mode === 'light' || mode === 'dark' ? mode : 'system';
  } catch {
    return 'system';
  }
}
/** Storage may be unavailable in a hardened webview; appearance still changes. */
export function saveTheme(mode: ThemeMode): void {
  try {
    localStorage.setItem('openrows.theme', mode);
  } catch {
    /* UI preferences remain usable without persistence. */
  }
}
