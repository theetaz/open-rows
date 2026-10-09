// SPDX-License-Identifier: Apache-2.0
import { DEFAULT_CUSTOM_ENV_COLOR } from '@openrows/ipc';
import {
  emptySsh,
  emptyTls,
  sshFromProfile,
  sshInput,
  tlsFromProfile,
  tlsInput,
  validateRoute,
} from './routeForm';
import type { RouteErrors, SshDraft, TlsDraft } from './routeForm';
import { defaultPort, engineName, isFileEngine } from './engines';
import type {
  EngineDto,
  EnvKindDto,
  PasswordInput,
  ProfileDto,
  ProfileInput,
  TlsModeDto,
  UrlDraft,
} from '@openrows/ipc';

/** How the password is handled on save; the value never comes back from the core. */
export type PasswordMode = 'keep' | 'set' | 'clear';

/** Editable connection form values (strings as typed). */
export type Draft = {
  /** The engine; fixed once the profile is saved. */
  engine: EngineDto;
  /** Editing a saved profile: a pasted URL may not change the engine. */
  engineLocked: boolean;
  name: string;
  group: string;
  envKind: EnvKindDto;
  envLabel: string;
  envColor: string;
  host: string;
  port: string;
  database: string;
  user: string;
  tlsMode: TlsModeDto;
  readOnly: boolean;
  confirmWrites: boolean;
  /** Keep executed statements in History (FR-WS-02). */
  recordHistory: boolean;
  password: string;
  passwordMode: PasswordMode;
  hasStoredPassword: boolean;
  /** The name was derived from the URL, so later URL edits may replace it. */
  nameFromUrl: boolean;
  /** Over SSH (D-10). */
  ssh: SshDraft;
  /** TLS certificate files (D-11); the mode is `tlsMode`. */
  tls: TlsDraft;
  /** The database file of a file engine (`SQLite`). */
  filePath: string;
  /** Open the file read-only. */
  fileReadOnly: boolean;
  /** Create the file when it does not exist ("New File…"). */
  fileCreate: boolean;
};

export type DraftErrors = Partial<
  Record<'name' | 'host' | 'port' | 'user' | 'envLabel' | 'filePath', string>
> &
  RouteErrors;

/** TextField `error` props for an optional message (exact optional property types). */
export function fieldError(message: string | null | undefined): {
  error?: string;
} {
  return message ? { error: message } : {};
}

export function emptyDraft(engine: EngineDto = 'postgres'): Draft {
  return {
    engine,
    engineLocked: false,
    name: '',
    group: '',
    envKind: 'local',
    envLabel: '',
    envColor: DEFAULT_CUSTOM_ENV_COLOR,
    host: 'localhost',
    port: String(defaultPort(engine) ?? ''),
    database: '',
    user: '',
    tlsMode: 'prefer',
    readOnly: false,
    confirmWrites: false,
    recordHistory: true,
    password: '',
    passwordMode: 'keep',
    hasStoredPassword: false,
    nameFromUrl: false,
    ssh: emptySsh(),
    tls: emptyTls(),
    filePath: '',
    fileReadOnly: false,
    fileCreate: false,
  };
}

export function draftFromProfile(profile: ProfileDto): Draft {
  return {
    ...emptyDraft(profile.engine),
    engineLocked: true,
    name: profile.name,
    group: profile.group ?? '',
    envKind: profile.env.kind,
    envLabel: profile.env.label ?? '',
    envColor:
      profile.env.kind === 'custom'
        ? profile.env.color
        : DEFAULT_CUSTOM_ENV_COLOR,
    host: profile.host,
    port: String(profile.port),
    database: profile.database ?? '',
    user: profile.user ?? '',
    tlsMode: profile.tlsMode,
    readOnly: profile.safety.readOnly,
    confirmWrites: profile.safety.confirmWrites,
    recordHistory: profile.recordHistory,
    hasStoredPassword: profile.hasPassword,
    ssh: sshFromProfile(profile),
    tls: tlsFromProfile(profile),
    filePath: profile.file?.path ?? '',
    fileReadOnly: profile.file?.readOnly ?? false,
    fileCreate: profile.file?.create ?? false,
  };
}

/** Switching to prod turns on confirm-before-write (FR-CONN-07). */
export function withEnvironment(draft: Draft, kind: EnvKindDto): Draft {
  return {
    ...draft,
    envKind: kind,
    confirmWrites: kind === 'prod' ? true : draft.confirmWrites,
  };
}

/** Why a parsed URL cannot fill this form, or `null` when it can. */
export function urlConflict(draft: Draft, parsed: UrlDraft): string | null {
  if (!draft.engineLocked || parsed.engine === draft.engine) return null;
  return `This is a ${engineName(parsed.engine)} URL; the connection uses ${engineName(draft.engine)}`;
}

/** The last path component, e.g. `app` for `/data/app.sqlite`. */
function fileStem(path: string): string {
  const base = path.split('/').pop() ?? path;
  return base.replace(/\.[^.]+$/, '') || base;
}

/** Fill connection fields from a parsed URL (its password is handled in the webview). */
export function withUrl(draft: Draft, parsed: UrlDraft): Draft {
  const derive = !draft.name || draft.nameFromUrl;
  const file = parsed.file;
  const derived = file ? fileStem(file.path) : parsed.database || parsed.host;
  return {
    ...draft,
    engine: parsed.engine,
    name: derive ? derived : draft.name,
    nameFromUrl: derive,
    host: parsed.host,
    port: String(parsed.port),
    database: parsed.database ?? '',
    user: parsed.user ?? '',
    tlsMode: parsed.tlsMode,
    filePath: file?.path ?? draft.filePath,
    fileReadOnly: file?.readOnly ?? draft.fileReadOnly,
    fileCreate: file ? false : draft.fileCreate,
  };
}

/** A field edit; typing a name stops the URL from renaming the connection. */
export function withPatch(draft: Draft, patch: Partial<Draft>): Draft {
  return {
    ...draft,
    ...patch,
    nameFromUrl: 'name' in patch ? false : draft.nameFromUrl,
  };
}

/** A typed password replaces the stored one; clearing the field keeps it. */
export function withPassword(draft: Draft, value: string): Draft {
  return { ...draft, password: value, passwordMode: value ? 'set' : 'keep' };
}

function validateFile(draft: Draft): DraftErrors {
  const errors: DraftErrors = {};
  const path = draft.filePath.trim();
  if (!draft.name.trim()) errors.name = 'Name is required';
  if (!path) errors.filePath = 'Choose a database file';
  else if (!path.startsWith('/')) errors.filePath = 'Use an absolute path';
  if (draft.envKind === 'custom' && !draft.envLabel.trim())
    errors.envLabel = 'Label is required';
  return errors;
}

export function validate(draft: Draft): DraftErrors {
  if (isFileEngine(draft.engine)) return validateFile(draft);
  const errors: DraftErrors = {};
  const port = Number(draft.port);
  if (!draft.name.trim()) errors.name = 'Name is required';
  if (!draft.host.trim()) errors.host = 'Host is required';
  if (!Number.isInteger(port) || port < 1 || port > 65535)
    errors.port = 'Port must be 1–65535';
  if (!draft.user.trim()) errors.user = 'User is required';
  if (draft.envKind === 'custom' && !draft.envLabel.trim())
    errors.envLabel = 'Label is required';
  return { ...errors, ...validateRoute(draft.ssh, draft.tls, draft.tlsMode) };
}

function password(draft: Draft): PasswordInput {
  switch (draft.passwordMode) {
    case 'set':
      return { kind: 'set', value: draft.password };
    case 'clear':
      return { kind: 'clear' };
    case 'keep':
      return { kind: 'keep' };
  }
}

const optional = (text: string): string | null => text.trim() || null;

/** The command payload for save and test. */
export function toInput(draft: Draft): ProfileInput {
  const custom = draft.envKind === 'custom';
  const file = isFileEngine(draft.engine);
  return {
    engine: draft.engine,
    file: file
      ? {
          path: draft.filePath.trim(),
          readOnly: draft.fileReadOnly,
          create: draft.fileCreate,
        }
      : null,
    name: draft.name.trim(),
    group: optional(draft.group),
    env: {
      kind: draft.envKind,
      label: custom ? optional(draft.envLabel) : null,
      color: custom ? draft.envColor : null,
    },
    host: file ? 'localhost' : draft.host.trim(),
    port: file ? 1 : Number(draft.port),
    database: optional(draft.database),
    user: optional(draft.user),
    tlsMode: draft.tlsMode,
    tlsFiles: tlsInput(draft.tls),
    ssh: file ? null : sshInput(draft.ssh),
    safety: { readOnly: draft.readOnly, confirmWrites: draft.confirmWrites },
    recordHistory: draft.recordHistory,
    password: file ? { kind: 'clear' } : password(draft),
  };
}
