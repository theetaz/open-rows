// SPDX-License-Identifier: Apache-2.0
import type { SettingsSection } from './SettingsSheet';
type Listener = (section: SettingsSection) => void;
let listener: Listener | null = null;
/** Ask the app shell to open Settings at a section, from anywhere in the workspace. */
export function openSettings(section: SettingsSection): void {
  listener?.(section);
}
/** The app shell owns the Settings sheet; returns the unsubscribe function. */
export function onSettingsRequest(next: Listener): () => void {
  listener = next;
  return () => {
    if (listener === next) listener = null;
  };
}
