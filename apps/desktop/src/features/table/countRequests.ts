// SPDX-License-Identifier: Apache-2.0
import { cancelQuery, tableCount } from '@openrows/ipc';
import type { TableQueryDto } from '@openrows/ipc';
import type { CountState } from './entries';

type Request = { sessionId: string; query: TableQueryDto };
type Context = {
  active: Map<string, string>;
  send: (tabId: string, count: CountState) => void;
};

/** One owned count per table; replacing or releasing it cancels its ticket. */
type CountRequests = {
  start: (tabId: string, key: string, request: Request) => void;
  cancel: (tabId: string) => void;
  release: (tabId: string) => void;
  clear: () => void;
};

let tickets = 0;

function cancel(context: Context, tabId: string): void {
  const ticket = context.active.get(tabId);
  if (ticket) cancelQuery(ticket).catch(() => undefined);
}

function finish(
  context: Context,
  tabId: string,
  ticket: string,
  count: CountState,
): void {
  if (context.active.get(tabId) !== ticket) return;
  context.active.delete(tabId);
  context.send(tabId, count);
}

function start(
  context: Context,
  tabId: string,
  key: string,
  request: Request,
): void {
  cancel(context, tabId);
  const ticket = `count-${String(++tickets)}`;
  context.active.set(tabId, ticket);
  context.send(tabId, { status: 'counting', key, ticket });
  tableCount({ ...request, ticket }).then(
    (reply) => {
      finish(
        context,
        tabId,
        ticket,
        reply.kind === 'counted' && reply.rows !== null
          ? { status: 'done', key, rows: reply.rows }
          : { status: 'idle' },
      );
    },
    (error: unknown) => {
      finish(context, tabId, ticket, {
        status: 'error',
        key,
        message: error instanceof Error ? error.message : String(error),
      });
    },
  );
}

/** Own count requests independently of render timing and ignore obsolete replies. */
export function countRequests(send: Context['send']): CountRequests {
  const context: Context = { active: new Map(), send };
  const release = (tabId: string): void => {
    cancel(context, tabId);
    context.active.delete(tabId);
  };
  return {
    start: (tabId, key, request) => {
      start(context, tabId, key, request);
    },
    cancel: (tabId) => {
      cancel(context, tabId);
    },
    release,
    clear: () => {
      for (const tabId of context.active.keys()) release(tabId);
    },
  };
}
