// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import { cancelAssistant, runAssistant } from '@openrows/ipc';
import type { AgentEvent, AssistantRequest, Trace } from '@openrows/ipc';
/** In-memory conversation; no payload or answer is persisted with tab descriptors. */
type Conversation = {
  open: boolean;
  running: string | null;
  turns: readonly Turn[];
};
type TimedTrace = Trace & { startedAt: number };
/** One user request and its streaming response and visible evidence trace. */
export type Turn = {
  id: string;
  question: string;
  text: string;
  proposals: readonly string[];
  traces: readonly TimedTrace[];
  confirmations: readonly Extract<AgentEvent, { type: 'confirmation' }>[];
  outcome: string;
};
const empty: Conversation = { open: false, running: null, turns: [] };
const states = new Map<string, Conversation>();
const listeners = new Set<() => void>();
function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
/** A tab's conversation survives switching tabs, within a bounded session cache. */
export function conversationOf(id: string): Conversation {
  return states.get(id) ?? empty;
}
function update(id: string, next: Conversation): void {
  const turns = [...next.turns];
  while (turns.length > 1 && JSON.stringify(turns).length > 65536)
    turns.shift();
  states.delete(id);
  states.set(id, { ...next, turns });
  for (const [key, value] of states) {
    if (states.size <= 32) break;
    if (!value.running) states.delete(key);
  }
  for (const listener of listeners) listener();
}
/** Drop a closed tab's memory and cancel any outstanding native work. */
export function clearConversation(id: string): void {
  cancelConversation(id);
  states.delete(id);
  for (const listener of listeners) listener();
}
/** Toggle the tab's dock without configuring providers or sending context. */
export function toggleAssistant(id: string): void {
  const state = conversationOf(id);
  if (state.open) cancelConversation(id);
  update(id, { ...conversationOf(id), open: !state.open });
}
/** Subscribe only to the active tab's immutable conversation snapshot. */
export function useConversation(id: string): Conversation {
  return useSyncExternalStore(subscribe, () => conversationOf(id));
}
/** Explicit cancellation also completes running trace rows immediately. */
export function cancelConversation(id: string): void {
  const state = conversationOf(id);
  if (!state.running) return;
  void cancelAssistant(state.running).catch(() => undefined);
  finish(id, state.running, 'Cancelled');
}
function finish(id: string, run: string, outcome: string): void {
  const state = conversationOf(id);
  if (state.running !== run) return;
  update(id, {
    ...state,
    running: null,
    turns: state.turns.map((turn) =>
      turn.id === run
        ? {
            ...turn,
            outcome,
            confirmations: [],
            traces: turn.traces.map((trace) =>
              trace.status === 'running'
                ? {
                    ...trace,
                    status: outcome,
                    durationMs: Math.min(
                      4294967295,
                      Math.max(
                        trace.durationMs,
                        Math.round(performance.now() - trace.startedAt),
                      ),
                    ),
                  }
                : trace,
            ),
          }
        : turn,
    ),
  });
}
function receive(id: string, run: string, event: AgentEvent): void {
  const state = conversationOf(id);
  if (state.running !== run) return;
  if (event.type === 'done') {
    finish(id, run, `Completed · ≤ ${String(event.tokens)} tokens`);
    return;
  }
  update(id, {
    ...state,
    turns: state.turns.map((turn) => {
      if (turn.id !== run) return turn;
      switch (event.type) {
        case 'confirmation':
          return {
            ...turn,
            confirmations: [...turn.confirmations, event].slice(0, 4),
          };
        case 'text':
          return { ...turn, text: (turn.text + event.delta).slice(0, 32768) };
        case 'proposal':
          if (
            turn.proposals.reduce((size, sql) => size + sql.length, 0) +
              event.sql.length >
            16384
          )
            return turn;
          return {
            ...turn,
            proposals: [...turn.proposals, event.sql].slice(0, 8),
          };
        case 'trace':
          return {
            ...turn,
            traces: [
              ...turn.traces.filter((trace) => trace.step !== event.trace.step),
              {
                ...event.trace,
                startedAt:
                  turn.traces.find((trace) => trace.step === event.trace.step)
                    ?.startedAt ?? performance.now(),
              },
            ].slice(0, 12),
          };
      }
    }),
  });
}
/** Begin only on a user action; stale/late events cannot overwrite a newer turn. */
export async function sendQuestion(
  id: string,
  request: AssistantRequest,
): Promise<void> {
  const state = conversationOf(id);
  if (state.running) return;
  const turn: Turn = {
    id: request.id,
    question: request.question.question,
    text: '',
    proposals: [],
    traces: [],
    confirmations: [],
    outcome: 'Running',
  };
  update(id, {
    ...state,
    running: request.id,
    turns: [...state.turns.slice(-3), turn],
  });
  try {
    await runAssistant(request, (event) => {
      receive(id, request.id, event);
    });
    finish(id, request.id, 'Completed');
  } catch (error) {
    finish(
      id,
      request.id,
      error instanceof Error ? error.message : 'Assistant failed',
    );
  }
}
