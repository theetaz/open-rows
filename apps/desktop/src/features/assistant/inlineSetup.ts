// SPDX-License-Identifier: Apache-2.0
import type { InlineBlock, InlineTransport } from '@openrows/editor';
import type { AiSettings, ProfileDto } from '@openrows/ipc';
import { aiSettings } from '../settings/ai/aiSettings';
import { openSettings } from '../settings/settingsRequest';
const action = {
  label: 'Open AI settings',
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
      reason:
        'AI features are off. Turn them on and add a provider in Settings → AI to write SQL with ⌘I.',
      action,
    };
  const provider = settings.providers.find(
    (item) =>
      item.id === (settings.defaultProvider || settings.providers[0]?.id),
  );
  if (!provider)
    return {
      reason:
        'No AI provider is set up. Add one in Settings → AI to write SQL with ⌘I.',
      action,
    };
  if (!settings.defaultModel)
    return {
      reason: `Choose a default model for ${provider.name} in Settings → AI.`,
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
      return 'This connection’s data-sharing policy does not allow this request. Allow the provider (Schema only or higher) under the connection’s Data sharing settings.';
    case 'auth':
      return 'The provider rejected its API key. Update the key in Settings → AI.';
    case 'timeout':
      return 'The provider did not answer in time. Try again.';
    case 'network':
      return 'The provider could not be reached. Check the network or base URL in Settings → AI.';
    case 'locked':
      return 'OpenRows is locked. Unlock it and try again.';
    default:
      return error instanceof Error && error.message
        ? error.message
        : 'Inline request failed.';
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
