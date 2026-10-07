// SPDX-License-Identifier: Apache-2.0
import { DEFAULT_CUSTOM_ENV_COLOR } from '@openrows/ipc';
import type {
  EnvKindDto,
  PasswordInput,
  ProfileDto,
  ProfileInput,
  TlsModeDto,
  UrlDraft,
} from '@openrows/ipc';

/** How the password is handled on save; the value never comes back from the core. */
export type PasswordMode = 'keep' | 'set' | 'clear' | 'url';

/** Editable connection form values (strings as typed). */
export type Draft = {
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
  password: string;
  passwordMode: PasswordMode;
  /** The pasted URL, kept only when it carried a password. */
  url: string;
  hasStoredPassword: boolean;
};

export type DraftErrors = Partial<
  Record<'name' | 'host' | 'port' | 'user' | 'envLabel', string>
>;

/** TextField `error` props for an optional message (exact optional property types). */
export function fieldError(message: string | null | undefined): {
  error?: string;
} {
  return message ? { error: message } : {};
}

export function emptyDraft(): Draft {
  return {
    name: '',
    group: '',
    envKind: 'local',
    envLabel: '',
    envColor: DEFAULT_CUSTOM_ENV_COLOR,
    host: 'localhost',
    port: '5432',
    database: '',
    user: '',
    tlsMode: 'prefer',
    readOnly: false,
    confirmWrites: false,
    password: '',
    passwordMode: 'keep',
    url: '',
    hasStoredPassword: false,
  };
}

export function draftFromProfile(profile: ProfileDto): Draft {
  return {
    ...emptyDraft(),
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
    hasStoredPassword: profile.hasPassword,
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

/** Fill connection fields from a parsed URL; the URL is kept only for its password. */
export function withUrl(draft: Draft, parsed: UrlDraft, url: string): Draft {
  return {
    ...draft,
    name: draft.name || parsed.database || parsed.host,
    host: parsed.host,
    port: String(parsed.port),
    database: parsed.database ?? '',
    user: parsed.user ?? draft.user,
    tlsMode: parsed.tlsMode,
    password: '',
    passwordMode: parsed.passwordProvided ? 'url' : draft.passwordMode,
    url: parsed.passwordProvided ? url : '',
  };
}

/** A typed password replaces the stored one; clearing the field keeps it. */
export function withPassword(draft: Draft, value: string): Draft {
  return {
    ...draft,
    password: value,
    passwordMode: value ? 'set' : 'keep',
    url: '',
  };
}

export function validate(draft: Draft): DraftErrors {
  const errors: DraftErrors = {};
  const port = Number(draft.port);
  if (!draft.name.trim()) errors.name = 'Name is required';
  if (!draft.host.trim()) errors.host = 'Host is required';
  if (!Number.isInteger(port) || port < 1 || port > 65535)
    errors.port = 'Port must be 1–65535';
  if (!draft.user.trim()) errors.user = 'User is required';
  if (draft.envKind === 'custom' && !draft.envLabel.trim())
    errors.envLabel = 'Label is required';
  return errors;
}

function password(draft: Draft): PasswordInput {
  switch (draft.passwordMode) {
    case 'set':
      return { kind: 'set', value: draft.password };
    case 'clear':
      return { kind: 'clear' };
    case 'url':
      return { kind: 'fromUrl', url: draft.url };
    case 'keep':
      return { kind: 'keep' };
  }
}

const optional = (text: string): string | null => text.trim() || null;

/** The command payload for save and test. */
export function toInput(draft: Draft): ProfileInput {
  const custom = draft.envKind === 'custom';
  return {
    name: draft.name.trim(),
    group: optional(draft.group),
    env: {
      kind: draft.envKind,
      label: custom ? optional(draft.envLabel) : null,
      color: custom ? draft.envColor : null,
    },
    host: draft.host.trim(),
    port: Number(draft.port),
    database: optional(draft.database),
    user: optional(draft.user),
    tlsMode: draft.tlsMode,
    safety: { readOnly: draft.readOnly, confirmWrites: draft.confirmWrites },
    password: password(draft),
  };
}
