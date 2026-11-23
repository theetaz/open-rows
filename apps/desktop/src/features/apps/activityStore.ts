// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import { listAgentActivity, onAgentActivity } from '@openrows/ipc';
import type { ActivityEvent } from '@openrows/ipc';

/** Events kept for the indicator and lists; the server keeps its own ring. */
const LIMIT = 100;

let events: readonly ActivityEvent[] = [];
let listening: Promise<unknown> | null = null;
let seeded = false;
const listeners = new Set<() => void>();

function publish(next: readonly ActivityEvent[]): void {
  events = next.slice(0, LIMIT);
  for (const listener of listeners) listener();
}

/** Start the live subscription once; it only listens to pushed events. */
function listen(): void {
  listening ??= onAgentActivity((event) => {
    // A call can arrive live after a seed that already included it.
    if (!events.some((known) => key(known) === key(event)))
      publish([event, ...events]);
  }).catch(() => {
    listening = null;
  });
}

function subscribe(listener: () => void): () => void {
  listen();
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Recent external tool calls, newest first, updated live. */
export function useAgentActivity(): readonly ActivityEvent[] {
  return useSyncExternalStore(subscribe, () => events);
}

/** Load the server's recent calls once (a user opened a list). */
export async function seedAgentActivity(): Promise<void> {
  if (seeded) return;
  seeded = true;
  try {
    const known = await listAgentActivity();
    const seen = new Set(events.map(key));
    publish([...events, ...known.filter((event) => !seen.has(key(event)))]);
  } catch {
    seeded = false;
  }
}

function key(event: ActivityEvent): string {
  return `${String(event.at)}|${event.token}|${event.tool}|${event.connection ?? ''}`;
}

/** Test hook: replace the store's events. */
export function setAgentActivityForTest(next: readonly ActivityEvent[]): void {
  seeded = false;
  publish(next);
}
