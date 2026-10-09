// SPDX-License-Identifier: Apache-2.0
import type { TlsModeDto } from '@openrows/ipc';

/** SQL Server's `Encrypt` choice, as its own tools name it. */
type Encrypt = 'mandatory' | 'optional' | 'login' | 'off';

/** Encrypt choices, strongest first. */
export const ENCRYPT_CHOICES: readonly { value: Encrypt; label: string }[] = [
  { value: 'mandatory', label: 'Mandatory' },
  { value: 'optional', label: 'Optional (when the server supports it)' },
  { value: 'login', label: 'Login only' },
  { value: 'off', label: 'Off (no TLS)' },
];

/** The Encrypt choice a TLS mode stands for. */
export function encryptOf(mode: TlsModeDto): Encrypt {
  switch (mode) {
    case 'disable':
      return 'off';
    case 'allow':
      return 'login';
    case 'prefer':
      return 'optional';
    default:
      return 'mandatory';
  }
}

/** Mandatory encryption that skips the certificate check. */
export function trustsCertificate(mode: TlsModeDto): boolean {
  return mode === 'require';
}

/**
 * The TLS mode for an Encrypt choice: Mandatory verifies the certificate
 * and host name unless the certificate is trusted; a CA-only check that is
 * already set stays.
 */
export function modeOf(
  encrypt: Encrypt,
  trust: boolean,
  current: TlsModeDto,
): TlsModeDto {
  switch (encrypt) {
    case 'off':
      return 'disable';
    case 'login':
      return 'allow';
    case 'optional':
      return 'prefer';
    case 'mandatory':
      if (trust) return 'require';
      return current === 'verify-ca' ? 'verify-ca' : 'verify-full';
  }
}

/** The section title, e.g. `Encrypt · Mandatory, trusted certificate`. */
export function encryptLabel(mode: TlsModeDto): string {
  const encrypt = encryptOf(mode);
  const label =
    ENCRYPT_CHOICES.find((choice) => choice.value === encrypt)?.label ??
    encrypt;
  if (encrypt !== 'mandatory') return label.replace(/ \(.*\)$/, '');
  return trustsCertificate(mode) ? `${label}, trusted certificate` : label;
}
