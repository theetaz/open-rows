// SPDX-License-Identifier: Apache-2.0
import type { ActivityOutcome, AgentApp } from '@openrows/ipc';
import type { Attention } from './appState';
import { t } from './messages';

const NAMES: Record<AgentApp, () => string> = {
  claude: () => t('claudeName'),
  codex: () => t('codexName'),
  gemini: () => t('geminiName'),
};
const BLURBS: Record<AgentApp, () => string> = {
  claude: () => t('claudeBlurb'),
  codex: () => t('codexBlurb'),
  gemini: () => t('geminiBlurb'),
};

const APPS: readonly AgentApp[] = ['claude', 'codex', 'gemini'];

/** Whether an activity or link identifier names a known app. */
export function isAgentApp(id: string | null): id is AgentApp {
  return APPS.includes(id as AgentApp);
}
export function appName(app: AgentApp): string {
  return NAMES[app]();
}
export function appBlurb(app: AgentApp): string {
  return BLURBS[app]();
}
/** The display name of an activity event's app, or a generic client label. */
export function clientName(id: string | null): string {
  return isAgentApp(id) ? appName(id) : t('unknownApp');
}

/** "just now", "4 min ago", "3 h ago", then the local date. */
export function relativeTime(at: number, now: number): string {
  const seconds = Math.max(0, now - at);
  if (seconds < 60) return t('justNow');
  if (seconds < 3600)
    return t('minutesAgo', { count: String(Math.floor(seconds / 60)) });
  if (seconds < 86_400)
    return t('hoursAgo', { count: String(Math.floor(seconds / 3600)) });
  return new Date(at * 1000).toLocaleDateString();
}

const OUTCOMES: Record<ActivityOutcome, () => string> = {
  ok: () => t('outcomeOk'),
  refused: () => t('outcomeRefused'),
  failed: () => t('outcomeFailed'),
};
export function outcomeLabel(outcome: ActivityOutcome): string {
  return OUTCOMES[outcome]();
}

/** One sentence per attention reason. */
export function reasonText(reason: Attention, detail = ''): string {
  switch (reason) {
    case 'missingTool':
      return t('reasonMissingTool');
    case 'revoked':
      return t('reasonRevoked');
    case 'expired':
      return t('reasonExpired');
    case 'serverOff':
      return t('reasonServerOff');
    case 'notRegistered':
      return t('reasonNotRegistered');
    case 'mismatch':
      return t('reasonMismatch');
    case 'unverified':
      return t('reasonUnverified', { message: detail });
  }
}

/** Seconds since the epoch, the unit the server reports. */
export function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}
