// SPDX-License-Identifier: Apache-2.0
import { commands, events } from './generated/bindings';
import type {
  IpcError,
  IpcErrorKind,
  ProfileDto,
  ProfileInput,
  ProfileList,
  SessionOpened,
  SessionStateChanged,
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
function toCommandError(error: unknown): CommandError {
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

/** Form fields from a `postgres://` URL; reports only whether it had a password. */
export function parsePostgresUrl(url: string): Promise<UrlDraft> {
  return unwrap(commands.profileParseUrl(url));
}

/** Connect once with the form values and report the server version. */
export function testConnection(
  profileId: string | null,
  input: ProfileInput,
): Promise<TestReport> {
  return unwrap(commands.connectionTest(profileId, input));
}

/** Open a session for a saved connection. */
export function openSession(profileId: string): Promise<SessionOpened> {
  return unwrap(commands.sessionOpen(profileId));
}

/** Disconnect a session. */
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
