// SPDX-License-Identifier: Apache-2.0
import { commandFields, commandErrors, commandDraft } from './credentialForm';
import type { HelperHop } from '@openrows/ipc';
import { helperError } from './helperForm';
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
import {
  emptyProxy,
  proxyErrors,
  proxyFromProfile,
  proxyInput,
} from './proxyForm';
import type { ProxyDraft, ProxyErrors } from './proxyForm';
import { emptyLimits, limitsFromProfile, limitsInput } from './limitsForm';
import type { LimitsDraft } from './limitsForm';
import { defaultPort, engineName, isFileEngine } from './engines';
import type {
  EngineDto,
  EnvKindDto,
  FileDto,
  PasswordInput,
  PasswordStorageDto,
  SafetyDto,
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
  mongodb?: { srv: boolean; authSource: string } | undefined;
  tlsMode: TlsModeDto;
  safety: SafetyDto;
  /** Keep executed statements in History (FR-WS-02). */
  recordHistory: boolean;
  /** Redact history even outside production/read-only contexts. */
  redactHistory: boolean;
  /** SQL run after every connect, reconnects included (FR-CONN-10). */
  startupCommands: string;
  password: string;
  passwordMode: PasswordMode;
  hasStoredPassword: boolean;
  /** Keychain, or asked on every connect (never saved). */
  passwordStorage: PasswordStorageDto;
  commandProgram: string;
  commandArgs: string;
  commandTimeout: string;
  tags: string;
  /** The name was derived from the URL, so later URL edits may replace it. */
  nameFromUrl: boolean;
  /** Over SSH (D-10). */
  ssh: SshDraft;
  /** Structured cloud route, independent of connection URLs. */
  helper: HelperHop | null;
  /** Through a SOCKS5 or HTTP proxy (M1-06). */
  proxy: ProxyDraft;
  /** TLS certificate files (D-11); the mode is `tlsMode`. */
  tls: TlsDraft;
  /** The database file of a file engine (`SQLite`). */
  filePath: string;
  /** An in-memory database instead of a file (no path, read-only or create). */
  fileMemory: boolean;
  /** Open the file read-only. */
  fileReadOnly: boolean;
  /** Create the file when it does not exist ("New File…"). */
  fileCreate: boolean;
  /** Statement and idle timeouts (M1-01). */
  limits: LimitsDraft;
};

export type DraftErrors = Partial<
  Record<
    | 'name'
    | 'host'
    | 'port'
    | 'user'
    | 'envLabel'
    | 'filePath'
    | 'commandProgram'
    | 'commandArgs'
    | 'commandTimeout'
    | 'helper',
    string
  >
> &
  RouteErrors &
  ProxyErrors;

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
    tlsMode:
      engine === 'clickhouse'
        ? 'disable'
        : engine === 'libsql'
          ? 'verify-full'
          : 'prefer',
    safety: 'unrestricted',
    recordHistory: true,
    redactHistory: false,
    startupCommands: '',
    password: '',
    passwordMode: 'keep',
    hasStoredPassword: false,
    passwordStorage: 'keychain',
    commandProgram: '',
    commandArgs: '[]',
    commandTimeout: '10',
    tags: '',
    nameFromUrl: false,
    ssh: emptySsh(),
    helper: null,
    proxy: emptyProxy(),
    tls: emptyTls(),
    filePath: '',
    fileMemory: false,
    fileReadOnly: false,
    fileCreate: false,
    limits: emptyLimits(),
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
    mongodb: mongoDraft(profile.mongodb),
    tlsMode: profile.tlsMode,
    safety: profile.safety,
    recordHistory: profile.recordHistory,
    redactHistory: profile.redactHistory,
    startupCommands: profile.startupCommands,
    hasStoredPassword: profile.hasPassword,
    passwordStorage: profile.passwordStorage,
    ...commandDraft(profile),
    tags: profile.tags.join(', '),
    ssh: sshFromProfile(profile),
    helper: profile.helper ?? null,
    proxy: proxyFromProfile(profile),
    tls: tlsFromProfile(profile),
    filePath: profile.file?.path ?? '',
    fileMemory: profile.file?.memory ?? false,
    fileReadOnly: profile.file?.readOnly ?? false,
    fileCreate: profile.file?.create ?? false,
    limits: limitsFromProfile(profile),
  };
}

/** Switching to prod turns on confirm-before-write (FR-CONN-07). */
export function withEnvironment(draft: Draft, kind: EnvKindDto): Draft {
  return {
    ...draft,
    envKind: kind,
    safety:
      kind === 'prod' && draft.safety === 'unrestricted'
        ? 'confirmWrites'
        : draft.safety,
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

/** A connection name for a parsed `sqlite:` URL. */
function fileName(file: FileDto): string {
  return file.memory ? 'In-memory' : fileStem(file.path);
}

/** Fill connection fields from a parsed URL (its password is handled in the webview). */
export function withUrl(draft: Draft, parsed: UrlDraft): Draft {
  const derive = !draft.name || draft.nameFromUrl;
  const file = parsed.file;
  const memory = file?.memory ?? false;
  const derived = file ? fileName(file) : parsed.database || parsed.host;
  return {
    ...draft,
    engine: parsed.engine,
    name: derive ? derived : draft.name,
    nameFromUrl: derive,
    host: parsed.host,
    port: String(parsed.port),
    database: parsed.database ?? '',
    user: parsed.user ?? '',
    mongodb: mongoDraft(parsed.mongodb),
    tlsMode: parsed.tlsMode,
    filePath: file && !memory ? file.path : draft.filePath,
    fileMemory: file ? memory : draft.fileMemory,
    fileReadOnly: file && !memory ? file.readOnly : draft.fileReadOnly,
    fileCreate: file ? false : draft.fileCreate,
  };
}

function mongoDraft(value: Draft['mongodb'] | null): Draft['mongodb'] {
  return value ?? undefined;
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

/** Why the file path cannot be used; an in-memory database needs none. */
function filePathError(draft: Draft): string | undefined {
  const path = draft.filePath.trim();
  if (draft.fileMemory) return undefined;
  if (!path) return 'Choose a database file';
  return path.startsWith('/') ? undefined : 'Use an absolute path';
}

function validateFile(draft: Draft): DraftErrors {
  const errors: DraftErrors = {};
  const pathError = filePathError(draft);
  if (!draft.name.trim()) errors.name = 'Name is required';
  if (pathError) errors.filePath = pathError;
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
  if (!['libsql', 'mongodb'].includes(draft.engine) && !draft.user.trim())
    errors.user = 'User is required';
  if (draft.envKind === 'custom' && !draft.envLabel.trim())
    errors.envLabel = 'Label is required';
  const cloudError = helperError(
    draft.helper,
    draft.ssh.enabled || draft.proxy.enabled || draft.host.startsWith('/'),
  );
  if (cloudError) errors.helper = cloudError;
  return {
    ...errors,
    ...commandErrors(draft),
    ...validateRoute(draft.ssh, draft.tls, draft.tlsMode),
    ...proxyErrors(draft.proxy, draft.host, draft.ssh.enabled),
  };
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

/** The file settings; an in-memory database has no path and no file options. */
function fileInput(draft: Draft): FileDto {
  const memory = draft.fileMemory;
  return {
    path: memory ? '' : draft.filePath.trim(),
    memory,
    readOnly: !memory && draft.fileReadOnly,
    create: !memory && draft.fileCreate,
  };
}

/** The command payload for save and test. */
export function toInput(draft: Draft): ProfileInput {
  const custom = draft.envKind === 'custom';
  const file = isFileEngine(draft.engine);
  return {
    engine: draft.engine,
    file: file ? fileInput(draft) : null,
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
    ...(draft.engine === 'mongodb'
      ? { mongodb: draft.mongodb ?? { srv: false, authSource: '' } }
      : {}),
    tlsMode: draft.tlsMode,
    tlsFiles: tlsInput(draft.tls),
    ssh: file ? null : sshInput(draft.ssh),
    helper: file ? null : draft.helper,
    proxy: file ? null : proxyInput(draft.proxy),
    safety: draft.safety,
    recordHistory: draft.recordHistory,
    redactHistory: draft.redactHistory,
    startupCommands: draft.startupCommands.trim(),
    passwordStorage: draft.passwordStorage,
    credentialCommand: commandFields(draft),
    tags: draft.tags
      .split(',')
      .map((tag) => tag.trim())
      .filter(Boolean),
    password:
      file ||
      draft.passwordStorage === 'none' ||
      draft.passwordStorage === 'command'
        ? { kind: 'clear' }
        : password(draft),
    limits: limitsInput(draft.limits),
  };
}
