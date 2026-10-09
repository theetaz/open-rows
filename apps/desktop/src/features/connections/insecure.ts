// SPDX-License-Identifier: Apache-2.0
import type { InsecureReasonDto, ProfileDto } from '@openrows/ipc';
import type { SessionStatus } from './sessions';

const REASON_TEXT: Record<InsecureReasonDto, string> = {
  tlsDisabled: 'TLS is off, so traffic to the server is readable',
  tlsUnverified: 'The server certificate is not verified',
  hostKeyNotSaved: 'The SSH host key was accepted once without saving it',
};

/**
 * Why a connection is insecure: the saved settings' reasons plus, while
 * connected, the live route's (a host key accepted once).
 */
export function insecureReasons(
  profile: ProfileDto,
  status: SessionStatus | undefined,
): InsecureReasonDto[] {
  const live = status?.state === 'connected' ? status.insecure : [];
  return [...new Set([...profile.insecure, ...live])];
}

/** One sentence per reason, for tooltips and screen readers. */
export function insecureDetail(reasons: readonly InsecureReasonDto[]): string {
  return reasons.map((reason) => `${REASON_TEXT[reason]}.`).join(' ');
}
