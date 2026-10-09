// SPDX-License-Identifier: Apache-2.0
import type {
  CloudCaDto,
  ProfileDto,
  SecretInput,
  SshAuthDto,
  SshInput,
  TlsFilesInput,
  TlsModeDto,
} from '@openrows/ipc';

/**
 * A keychain secret in the form: typed value, what Save does with it, and
 * whether one is stored (its value never comes back from the core).
 */
export type SecretDraft = {
  value: string;
  mode: 'keep' | 'set' | 'clear';
  stored: boolean;
};

export type SshAuthKind = SshAuthDto['method'];

/** "Over SSH" section values, strings as typed. */
export type SshDraft = {
  enabled: boolean;
  host: string;
  port: string;
  user: string;
  auth: SshAuthKind;
  keyFile: string;
  /** Comma-separated `[user@]host[:port]` jump hosts, first hop first. */
  jumpHosts: string;
  password: SecretDraft;
  passphrase: SecretDraft;
};

/** TLS certificate files and the client key passphrase. */
export type TlsDraft = {
  caFile: string;
  /** A bundled cloud provider CA instead of a CA file. */
  cloudCa: CloudCaDto | null;
  clientCert: string;
  clientKey: string;
  passphrase: SecretDraft;
};

export type RouteErrors = Partial<
  Record<
    | 'sshHost'
    | 'sshPort'
    | 'sshKeyFile'
    | 'sshJumpHosts'
    | 'tlsCaFile'
    | 'tlsClientKey',
    string
  >
>;

export function emptySecret(stored = false): SecretDraft {
  return { value: '', mode: 'keep', stored };
}

export function emptySsh(): SshDraft {
  return {
    enabled: false,
    host: '',
    port: '',
    user: '',
    auth: 'agent',
    keyFile: '',
    jumpHosts: '',
    password: emptySecret(),
    passphrase: emptySecret(),
  };
}

export function emptyTls(): TlsDraft {
  return {
    caFile: '',
    cloudCa: null,
    clientCert: '',
    clientKey: '',
    passphrase: emptySecret(),
  };
}

export function sshFromProfile(profile: ProfileDto): SshDraft {
  const ssh = profile.ssh;
  if (!ssh) return emptySsh();
  return {
    enabled: true,
    host: ssh.host,
    port: ssh.port === null ? '' : String(ssh.port),
    user: ssh.user ?? '',
    auth: ssh.auth.method,
    keyFile: ssh.auth.method === 'privateKey' ? ssh.auth.keyFile : '',
    jumpHosts: ssh.jumpHosts.join(', '),
    password: emptySecret(ssh.hasPassword),
    passphrase: emptySecret(ssh.hasPassphrase),
  };
}

export function tlsFromProfile(profile: ProfileDto): TlsDraft {
  const files = profile.tlsFiles;
  return {
    caFile: files.caFile ?? '',
    cloudCa: files.cloudCa,
    clientCert: files.clientCert ?? '',
    clientKey: files.clientKey ?? '',
    passphrase: emptySecret(files.hasKeyPassphrase),
  };
}

/** A typed secret replaces the stored one; clearing the field keeps it. */
export function withSecret(secret: SecretDraft, value: string): SecretDraft {
  return { ...secret, value, mode: value ? 'set' : 'keep' };
}

export function secretInput(secret: SecretDraft): SecretInput {
  switch (secret.mode) {
    case 'set':
      return { kind: 'set', value: secret.value };
    case 'clear':
      return { kind: 'clear' };
    case 'keep':
      return { kind: 'keep' };
  }
}

const optional = (text: string): string | null => text.trim() || null;

/** Jump hosts as entered, split on commas and line breaks. */
export function jumpList(text: string): string[] {
  return text
    .split(/[,\n]/)
    .map((jump) => jump.trim())
    .filter(Boolean);
}

function sshAuth(ssh: SshDraft): SshAuthDto {
  return ssh.auth === 'privateKey'
    ? { method: 'privateKey', keyFile: ssh.keyFile.trim() }
    : { method: ssh.auth };
}

/** The command payload for the SSH section; `null` when SSH is off. */
export function sshInput(ssh: SshDraft): SshInput | null {
  if (!ssh.enabled) return null;
  return {
    host: ssh.host.trim(),
    port: ssh.port.trim() ? Number(ssh.port) : null,
    user: optional(ssh.user),
    auth: sshAuth(ssh),
    jumpHosts: jumpList(ssh.jumpHosts),
    password: secretInput(ssh.password),
    passphrase: secretInput(ssh.passphrase),
  };
}

export function tlsInput(tls: TlsDraft): TlsFilesInput {
  return {
    caFile: tls.cloudCa ? null : optional(tls.caFile),
    cloudCa: tls.cloudCa,
    clientCert: optional(tls.clientCert),
    clientKey: optional(tls.clientKey),
    keyPassphrase: secretInput(tls.passphrase),
  };
}

const JUMP = /^(?:[^@\s,]+@)?(?:\[[^\]\s]+\]|[^\s:@,[\]]+)(?::\d{1,5})?$/;

export function validPort(text: string): boolean {
  const port = Number(text);
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}

function sshErrors(ssh: SshDraft): RouteErrors {
  if (!ssh.enabled) return {};
  const errors: RouteErrors = {};
  if (!ssh.host.trim()) errors.sshHost = 'SSH host is required';
  if (ssh.port.trim() && !validPort(ssh.port))
    errors.sshPort = 'Port must be 1–65535';
  if (ssh.auth === 'privateKey' && !ssh.keyFile.trim())
    errors.sshKeyFile = 'Choose a private key file';
  if (jumpList(ssh.jumpHosts).some((jump) => !JUMP.test(jump)))
    errors.sshJumpHosts = 'Use [user@]host[:port], separated by commas';
  return errors;
}

function tlsErrors(tls: TlsDraft, mode: TlsModeDto): RouteErrors {
  const errors: RouteErrors = {};
  if (mode === 'verify-ca' && !tlsCa(tls))
    errors.tlsCaFile = 'Verify CA needs a CA certificate';
  if (Boolean(tls.clientCert.trim()) !== Boolean(tls.clientKey.trim()))
    errors.tlsClientKey = 'Choose both a client certificate and its key';
  return errors;
}

/** The configured trust anchor (CA file or cloud CA), or `''` for the system store. */
export function tlsCa(tls: TlsDraft): string {
  return tls.cloudCa ?? tls.caFile.trim();
}

export function validateRoute(
  ssh: SshDraft,
  tls: TlsDraft,
  mode: TlsModeDto,
): RouteErrors {
  return { ...sshErrors(ssh), ...tlsErrors(tls, mode) };
}

const LOCAL_HOSTS = new Set(['localhost', '127.0.0.1', '::1']);

/**
 * True when the server certificate of a remote host is not checked (libpq:
 * only verify-ca, verify-full and require with a CA file check certificates).
 */
function unverifiedRemote(
  host: string,
  mode: TlsModeDto,
  caFile: string,
): boolean {
  const trimmed = host.trim();
  if (LOCAL_HOSTS.has(trimmed) || trimmed.startsWith('/')) return false;
  if (mode === 'verify-ca' || mode === 'verify-full') return false;
  return !(mode === 'require' && caFile.trim());
}

type TlsWarning = { badge: string; detail: string };

/** The persistent flag for a remote connection without certificate checks. */
export function tlsWarning(
  host: string,
  mode: TlsModeDto,
  caFile: string,
): TlsWarning | null {
  if (!unverifiedRemote(host, mode, caFile)) return null;
  if (mode === 'disable')
    return {
      badge: 'Not encrypted',
      detail:
        'TLS is off: traffic to this remote server is readable on the network.',
    };
  return {
    badge: 'Certificate not verified',
    detail:
      'The server identity is not checked, so the connection can be intercepted. Use Verify full, or Verify CA with the server CA file.',
  };
}

/** Collapsed SSH section summary: `Off`, or `user@host:port`. */
export function sshSummary(ssh: SshDraft): string {
  if (!ssh.enabled) return 'Off';
  const user = ssh.user.trim() ? `${ssh.user.trim()}@` : '';
  const port = ssh.port.trim() ? `:${ssh.port.trim()}` : '';
  return `${user}${ssh.host.trim() || 'no host'}${port}`;
}
