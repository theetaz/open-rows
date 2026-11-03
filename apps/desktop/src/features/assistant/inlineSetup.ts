// SPDX-License-Identifier: Apache-2.0
import type { InlineBlock, InlineTransport } from '@openrows/editor';
import type { AiSettings, ProfileDto } from '@openrows/ipc';
import { aiSettings } from '../settings/ai/aiSettings';
import { openSettings } from '../settings/settingsRequest';
import { t } from './lazyMessages';
const action = {
  label: t('openAiSettings'),
  run: () => {
    openSettings('ai');
  },
};
/** Why ⌘I cannot generate yet; checked before any prompt is accepted. */
export function inlineBlock(
  settings: Required<AiSettings>,
): InlineBlock | null {
  if (!settings.enabled)
    return {
      reason: t('aiFeaturesOffTurn'),
      action,
    };
  const provider = settings.providers.find(
    (item) =>
      item.id === (settings.defaultProvider || settings.providers[0]?.id),
  );
  if (!provider)
    return {
      reason: t('noAiProvider'),
      action,
    };
  if (!settings.defaultModel)
    return {
      reason: t('chooseDefaultModel', { name: provider.name }),
      action,
    };
  return null;
}
/** Native refusals arrive as typed command errors; explain the fix, not the code. */
export function inlineFailure(error: unknown): string {
  const kind =
    error && typeof error === 'object' && 'kind' in error
      ? String(error.kind)
      : '';
  switch (kind) {
    case 'unsupported':
      return t('connectionSData');
    case 'auth':
      return t('providerRejectedApi');
    case 'timeout':
      return t('providerDidNotAnswer');
    case 'network':
      return t('providerCouldNot');
    case 'locked':
      return t('openrowsLockedUnlock');
    default:
      return error instanceof Error && error.message
        ? error.message
        : t('inlineRequestFailed');
  }
}
/** ⌘I transport for a query tab; the generator and its IPC load on first use. */
export function inlineTransport(scope: {
  profile: ProfileDto;
  database: string;
}): InlineTransport {
  return {
    blocked: () => inlineBlock(aiSettings.get()),
    generate: (input) =>
      import('./inline').then(({ inlineGenerator }) =>
        inlineGenerator(scope)(input),
      ),
  };
}
