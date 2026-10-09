// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import type { ExplainRequest, PlanDto } from '@openrows/ipc';

/** One query tab's latest explain. */
export type ExplainState =
  | { status: 'running'; analyze: boolean; explainId: string }
  /** Explain Analyze of a possible write waits for the user's consent. */
  | {
      status: 'confirm';
      request: ExplainRequest;
      token: string;
      statement: string;
    }
  | { status: 'done'; analyze: boolean; plan: PlanDto; explainId: string }
  | { status: 'failed'; analyze: boolean; message: string };

/** Plans of recently used tabs are kept; older ones are dropped. */
const KEPT = 32;

const states = new Map<string, ExplainState>();
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The tab's explain, or `null` when it has none. */
export function explainOf(tabId: string): ExplainState | null {
  return states.get(tabId) ?? null;
}

/** Replace (or with `null` drop) the tab's explain. */
export function setExplain(tabId: string, state: ExplainState | null): void {
  states.delete(tabId);
  if (state) states.set(tabId, state);
  for (const key of states.keys()) {
    if (states.size <= KEPT) break;
    states.delete(key);
  }
  for (const listener of listeners) listener();
}

/**
 * The tab's explain, kept outside the tab component so it survives
 * switching tabs.
 */
export function useExplainState(tabId: string): ExplainState | null {
  return useSyncExternalStore(subscribe, () => explainOf(tabId));
}
