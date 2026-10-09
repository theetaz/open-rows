// SPDX-License-Identifier: Apache-2.0
import { useCallback } from 'react';
import { cancelQuery, explainQuery } from '@openrows/ipc';
import type { ExplainReply, ExplainRequest } from '@openrows/ipc';
import { explainOf, setExplain, useExplainState } from './explainStore';
import type { ExplainState } from './explainStore';

/** The editor text an explain reads. */
export type ExplainSource = {
  text: () => string;
  /** Cursor as a UTF-16 offset (CodeMirror position). */
  cursor: () => number;
  selection: () => string;
};

/** A query tab's explain and its actions. */
export type ExplainControls = {
  state: ExplainState | null;
  /** Explain (or with `analyze` measure) the selection, else the statement at the cursor. */
  run: (analyze: boolean) => void;
  /** Run the Explain Analyze waiting for consent. */
  confirm: () => void;
  cancel: () => void;
  /** Drop the plan (and cancel a running explain). */
  close: () => void;
};

/** The request explaining the selection, else the statement at the cursor. */
export function explainRequest(
  source: ExplainSource,
  sessionId: string,
  analyze: boolean,
): ExplainRequest | null {
  const base = {
    sessionId,
    explainId: crypto.randomUUID(),
    analyze,
    confirmToken: null,
  };
  const selection = source.selection();
  if (selection.trim()) return { ...base, sql: selection, cursor: null };
  const sql = source.text();
  if (!sql.trim()) return null;
  return { ...base, sql, cursor: source.cursor() };
}

function stateOf(
  reply: ExplainReply,
  request: ExplainRequest,
): ExplainState | null {
  const { analyze, explainId } = request;
  switch (reply.kind) {
    case 'explained':
      return { status: 'done', analyze, plan: reply.plan, explainId };
    case 'confirmationRequired':
      return {
        status: 'confirm',
        request,
        token: reply.token,
        statement: reply.statement,
      };
    case 'blocked':
      return { status: 'failed', analyze, message: reply.message };
    case 'cancelled':
      return null;
  }
}

/** Send `request`; a reply only lands while it is still the tab's explain. */
function send(tabId: string, request: ExplainRequest): void {
  const { analyze, explainId } = request;
  setExplain(tabId, { status: 'running', analyze, explainId });
  const current = (): boolean => {
    const state = explainOf(tabId);
    return state?.status === 'running' && state.explainId === explainId;
  };
  explainQuery(request).then(
    (reply) => {
      if (current()) setExplain(tabId, stateOf(reply, request));
    },
    (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      if (current()) setExplain(tabId, { status: 'failed', analyze, message });
    },
  );
}

/** Explain commands of one query tab; the plan outlives switching tabs. */
export function useExplain(
  tabId: string,
  sessionId: string | null,
  editor: { current: ExplainSource | null },
): ExplainControls {
  const state = useExplainState(tabId);
  const run = (analyze: boolean): void => {
    const source = editor.current;
    if (!source || !sessionId || explainOf(tabId)?.status === 'running') return;
    const request = explainRequest(source, sessionId, analyze);
    if (request) send(tabId, request);
  };
  const cancel = useCallback(() => {
    const current = explainOf(tabId);
    if (current?.status === 'running') void cancelQuery(current.explainId);
  }, [tabId]);
  return {
    state,
    run,
    confirm: () => {
      const current = explainOf(tabId);
      if (current?.status !== 'confirm') return;
      send(tabId, {
        ...current.request,
        explainId: crypto.randomUUID(),
        confirmToken: current.token,
      });
    },
    cancel,
    close: () => {
      cancel();
      setExplain(tabId, null);
    },
  };
}
