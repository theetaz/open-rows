// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import { openSettings } from '../settingsRequest';

/** The pages of Settings → AI. */
export type AiPage = 'providers' | 'apps';

let page: AiPage = 'providers';
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Switch Settings → AI to a page; an open pane follows at once. */
export function setAiPage(next: AiPage): void {
  page = next;
  for (const listener of listeners) listener();
}

/** The current Settings → AI page. */
export function useAiPage(): AiPage {
  return useSyncExternalStore(subscribe, () => page);
}

/** Open Settings at AI → Apps from anywhere (assistant, toolbar indicator). */
export function showAgentApps(): void {
  setAiPage('apps');
  openSettings('ai');
}
