// SPDX-License-Identifier: Apache-2.0
import type {
  ColumnDto,
  ExecuteReply,
  ExecuteRequest,
  QueryEventDto,
  StatementDto,
} from '@openrows/ipc';

/** A result set's shape and size; its rows stay in the core's result store. */
export type ResultMeta = {
  id: string;
  statement: number;
  columns: readonly ColumnDto[];
  /** Trailing store columns that views never show (a keyless table's rowid). */
  hidden?: readonly ColumnDto[];
  rows: number;
  done: boolean;
  truncated: boolean;
};

export type RunMessage = {
  statement: number;
  level: 'info' | 'warning' | 'error';
  text: string;
  code: string | null;
};

export type RunStatus =
  | 'idle'
  | 'starting'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'confirm'
  | 'parameters'
  | 'blocked';

/** One tab's latest run: status, result metadata, messages; never row data. */
export type RunState = {
  status: RunStatus;
  queryId: string | null;
  statements: readonly StatementDto[];
  results: readonly ResultMeta[];
  messages: readonly RunMessage[];
  affected: number | null;
  done: number;
  elapsedMs: number | null;
  /** The request to resend with `token` once the user confirms. */
  pending: { request: ExecuteRequest; token: string } | null;
  /** The request to resend once the user entered these parameter values. */
  parameters: { request: ExecuteRequest; names: readonly string[] } | null;
};

export const idleRun: RunState = {
  status: 'idle',
  queryId: null,
  statements: [],
  results: [],
  messages: [],
  affected: null,
  done: 0,
  elapsedMs: null,
  pending: null,
  parameters: null,
};

export type RunAction =
  | { type: 'begin' }
  | { type: 'reply'; reply: ExecuteReply; request: ExecuteRequest }
  /** A generated statement (table page) started without a safety plan. */
  | { type: 'started'; queryId: string }
  | { type: 'rejected'; message: string; code: string | null }
  | { type: 'event'; event: QueryEventDto }
  | { type: 'dismiss' };

export function runReducer(state: RunState, action: RunAction): RunState {
  switch (action.type) {
    case 'begin':
      return { ...idleRun, status: 'starting' };
    case 'reply':
      return applyReply(state, action.reply, action.request);
    case 'started':
      return {
        ...state,
        queryId: action.queryId,
        status: state.status === 'starting' ? 'running' : state.status,
      };
    case 'rejected':
      return {
        ...idleRun,
        status: 'failed',
        messages: [error(0, action.message, action.code)],
      };
    case 'event':
      return applyEvent(state, action.event);
    case 'dismiss':
      return { ...state, status: 'idle', pending: null, parameters: null };
  }
}

function error(
  statement: number,
  text: string,
  code: string | null,
): RunMessage {
  return { statement, level: 'error', text, code };
}

function applyReply(
  state: RunState,
  reply: ExecuteReply,
  request: ExecuteRequest,
): RunState {
  switch (reply.kind) {
    case 'started':
      return {
        ...state,
        statements: reply.statements,
        queryId: reply.queryId,
        status: state.status === 'starting' ? 'running' : state.status,
      };
    case 'confirmationRequired':
      return {
        ...idleRun,
        status: 'confirm',
        statements: reply.statements,
        pending: { request, token: reply.token },
      };
    case 'blocked':
      return {
        ...idleRun,
        status: 'blocked',
        statements: reply.statements,
        messages: [error(0, reply.message, null)],
      };
    case 'parametersRequired':
      return {
        ...idleRun,
        status: 'parameters',
        parameters: { request, names: reply.names },
      };
  }
}

function applyEvent(state: RunState, event: QueryEventDto): RunState {
  switch (event.type) {
    case 'statementStarted':
      return state.status === 'starting'
        ? { ...state, status: 'running' }
        : state;
    case 'resultBegin':
      return {
        ...state,
        results: [
          ...state.results,
          {
            id: event.resultId,
            statement: event.statement,
            columns: event.columns,
            rows: 0,
            done: false,
            truncated: false,
          },
        ],
      };
    case 'progress':
      return updateResult(state, event.resultId, { rows: event.rows });
    case 'resultEnd':
      return updateResult(state, event.resultId, {
        rows: event.rows,
        truncated: event.truncated,
        done: true,
      });
    default:
      return applyOutcome(state, event);
  }
}

function applyOutcome(state: RunState, event: QueryEventDto): RunState {
  switch (event.type) {
    case 'rowsAffected':
      return { ...state, affected: (state.affected ?? 0) + event.count };
    case 'notice':
      return {
        ...state,
        messages: [
          ...state.messages,
          {
            statement: event.statement,
            level: event.level,
            text: event.message,
            code: null,
          },
        ],
      };
    case 'statementDone':
      return { ...state, done: state.done + 1 };
    case 'failed':
      return {
        ...state,
        messages: [
          ...state.messages,
          error(event.statement, event.error.message, event.error.code),
        ],
      };
    case 'finished':
      return { ...state, status: event.outcome, elapsedMs: event.elapsedMs };
    default:
      return state;
  }
}

function updateResult(
  state: RunState,
  id: string,
  change: Partial<ResultMeta>,
): RunState {
  return {
    ...state,
    results: state.results.map((result) =>
      result.id === id ? { ...result, ...change } : result,
    ),
  };
}

/** True while statements may still produce events. */
export function isActive(run: RunState): boolean {
  return run.status === 'starting' || run.status === 'running';
}
