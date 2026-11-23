// SPDX-License-Identifier: Apache-2.0
import type { AgentApp, AppCard, AppCheck } from '@openrows/ipc';

/** The tool's check of its `openrows` entry, which runs after the page loads. */
export type CheckState =
  | { kind: 'pending' }
  | { kind: 'done'; check: AppCheck }
  | { kind: 'failed'; message: string };

export type AppStatus =
  'notInstalled' | 'installed' | 'checking' | 'connected' | 'attention';

/** Why a connected app needs attention, most actionable first. */
export type Attention =
  | 'missingTool'
  | 'revoked'
  | 'expired'
  | 'serverOff'
  | 'notRegistered'
  | 'mismatch'
  | 'unverified';

export type Derived = { status: AppStatus; reasons: Attention[] };

/** Three days: Details offers Renew from here on. */
const RENEW_WINDOW = 3 * 86_400;

function linkReasons(
  card: AppCard,
  port: number | null,
  now: number,
): Attention[] {
  const reasons: Attention[] = [];
  const scope = card.link?.scope;
  if (!card.install) reasons.push('missingTool');
  if (!scope) reasons.push('revoked');
  else if (scope.expires <= now) reasons.push('expired');
  if (port === null) reasons.push('serverOff');
  return reasons;
}

function checkReasons(check: CheckState | undefined): Attention[] {
  if (!check || check.kind === 'pending') return [];
  if (check.kind === 'failed') return ['unverified'];
  const { registration, expected } = check.check;
  if (!registration.present) return ['notRegistered'];
  if (expected !== null && registration.line !== expected) return ['mismatch'];
  return [];
}

/** Combine detection, the local link and the tool's own check into one state. */
export function deriveStatus(
  card: AppCard,
  check: CheckState | undefined,
  port: number | null,
  now: number,
): Derived {
  if (!card.link)
    return { status: card.install ? 'installed' : 'notInstalled', reasons: [] };
  const reasons = [...linkReasons(card, port, now), ...checkReasons(check)];
  if (reasons.length) return { status: 'attention', reasons };
  const pending = !check || check.kind === 'pending';
  return { status: pending ? 'checking' : 'connected', reasons };
}

/** Whether Details should offer Renew for a working connection. */
export function renewable(card: AppCard, now: number): boolean {
  const expires = card.link?.scope?.expires;
  return expires !== undefined && expires - now < RENEW_WINDOW;
}

/** Preferred saved-provider kinds per app, used to preselect the scope. */
const PROVIDER_KINDS: Record<AgentApp, readonly string[]> = {
  claude: ['anthropic'],
  codex: ['codex', 'openAi'],
  gemini: ['gemini'],
};

/** The provider to preselect: a matching kind, else the default, else the first. */
export function preferredProvider(
  app: AgentApp,
  providers: readonly { id: string; kind: string }[],
  fallback: string,
): string {
  const kinds = PROVIDER_KINDS[app];
  return (
    providers.find((provider) => kinds.includes(provider.kind))?.id ??
    providers.find((provider) => provider.id === fallback)?.id ??
    providers[0]?.id ??
    ''
  );
}
