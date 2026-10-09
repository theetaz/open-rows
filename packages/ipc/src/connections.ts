// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands, events } from './generated/bindings';
import type {
  AttachReply,
  ConnectionOpened,
  IpcError,
  IpcErrorKind,
  PickFileKind,
  ProfileDto,
  ProfileInput,
  ProfileList,
  SessionOpened,
  SessionStateChanged,
  SshPromptAnswer,
  SshPromptChanged,
  TestReport,
  UrlDraft,
} from './generated/bindings';

/** A rejected connection command with its typed category and SQLSTATE. */
export class CommandError extends Error {
  readonly kind: IpcErrorKind;
  readonly code: string | null;

  constructor(error: IpcError) {
    super(error.message);
    this.name = 'CommandError';
    this.kind = error.kind;
    this.code = error.code;
  }
}

type Result<T> =
  { status: 'ok'; data: T } | { status: 'error'; error: unknown };

/** Tauri rejects with plain strings for permission or transport failures. */
export function toCommandError(error: unknown): CommandError {
  if (typeof error === 'object' && error !== null && 'kind' in error)
    return new CommandError(error as IpcError);
  const message = error instanceof Error ? error.message : String(error);
  return new CommandError({ kind: 'internal', message, code: null });
}

export async function unwrap<T>(pending: Promise<Result<T>>): Promise<T> {
  const result = await pending;
  if (result.status === 'error') throw toCommandError(result.error);
  return result.data;
}

/** Saved connections, sorted by group then name. */
export function listProfiles(): Promise<ProfileList> {
  return unwrap(commands.profilesList());
}

/** Saved connections matching every whitespace-separated term. */
export function searchProfiles(query: string): Promise<ProfileDto[]> {
  return unwrap(commands.profilesSearch(query));
}

/** One saved connection. */
export function getProfile(id: string): Promise<ProfileDto> {
  return unwrap(commands.profileGet(id));
}

/** Save a new connection; a password goes to the OS keychain only. */
export function createProfile(input: ProfileInput): Promise<ProfileDto> {
  return unwrap(commands.profileCreate(input));
}

/** Replace a saved connection and apply its password change. */
export function updateProfile(
  id: string,
  input: ProfileInput,
): Promise<ProfileDto> {
  return unwrap(commands.profileUpdate(id, input));
}

/** Copy a connection, including its stored password, as "<name> copy". */
export function duplicateProfile(id: string): Promise<ProfileDto> {
  return unwrap(commands.profileDuplicate(id));
}

/** Delete a connection and its stored password. */
export async function deleteProfile(id: string): Promise<void> {
  await unwrap(commands.profileDelete(id));
}

/** Form fields from a `postgres://`, `mysql://`, `mariadb://` or `sqlite:` URL; reports only whether it had a password. */
export function parseConnectionUrl(url: string): Promise<UrlDraft> {
  return unwrap(commands.profileParseUrl(url));
}

/**
 * Connect once with the form values, step by step (SSH, tunnel, TLS,
 * authentication). Resolves with the steps and either the server or the
 * failing step's typed error; rejects only for invalid input.
 */
export function testConnection(
  profileId: string | null,
  input: ProfileInput,
): Promise<TestReport> {
  return unwrap(commands.connectionTest(profileId, input));
}

/** Reach a saved connection's server; databases open with `openSession`. */
export function openConnection(profileId: string): Promise<ConnectionOpened> {
  return unwrap(commands.connectionOpen(profileId));
}

/** Close every database session and metadata session of a connection. */
export async function closeConnection(profileId: string): Promise<void> {
  await unwrap(commands.connectionClose(profileId));
}

/** Open a session on one database (the profile's database when `null`). */
export function openSession(
  profileId: string,
  database: string | null,
): Promise<SessionOpened> {
  return unwrap(commands.sessionOpen(profileId, database));
}

/** Disconnect a database session (and its metadata session with the last one). */
export async function closeSession(sessionId: string): Promise<void> {
  await unwrap(commands.sessionClose(sessionId));
}

/** Subscribe to session state changes; resolves to the unsubscribe function. */
export function onSessionState(
  listener: (event: SessionStateChanged) => void,
): Promise<() => void> {
  return events.sessionStateChanged.listen((event) => {
    listener(event.payload);
  });
}

/** Pick a key, certificate or database file natively; `null` when cancelled or outside the shell. */
export async function pickConnectionFile(
  kind: PickFileKind,
): Promise<string | null> {
  if (!isTauri()) return null;
  return unwrap(commands.connectionPickFile(kind));
}

/**
 * Attach an existing `SQLite` file as schema `alias` to a saved connection
 * and its open sessions; resolves with the saved profile and a warning per
 * open session that could not follow (it does once reopened).
 */
export function attachDatabase(
  profileId: string,
  path: string,
  alias: string,
): Promise<AttachReply> {
  return unwrap(commands.connectionAttachDatabase(profileId, path, alias));
}

/** Detach schema `alias` from a saved `SQLite` connection and its open sessions. */
export function detachDatabase(
  profileId: string,
  alias: string,
): Promise<AttachReply> {
  return unwrap(commands.connectionDetachDatabase(profileId, alias));
}

/** Answer an SSH host-key or keyboard-interactive prompt (answers go to the core only). */
export async function answerSshPrompt(
  id: string,
  answer: SshPromptAnswer,
): Promise<void> {
  await unwrap(commands.sshPromptAnswer(id, answer));
}

/** Subscribe to SSH prompts opening and closing; resolves to the unsubscribe function. */
export function onSshPrompt(
  listener: (event: SshPromptChanged) => void,
): Promise<() => void> {
  if (!isTauri()) return Promise.resolve(() => {});
  return events.sshPromptChanged.listen((event) => {
    listener(event.payload);
  });
}
