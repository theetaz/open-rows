// SPDX-License-Identifier: Apache-2.0
import type { AiSettings, ProviderConfig, ProviderKind } from '@openrows/ipc';
import { localSetting } from '../localSetting';
/** Settings-file section; only non-secret fields are copied. */
export const AI_KEY = 'openrows.ai.v1';
/** Available protocols and endpoint suggestions; no hard-coded model catalogue. */
export const PROVIDER_KINDS: readonly {
  value: ProviderKind;
  label: string;
  url: string;
}[] = [
  {
    value: 'anthropic',
    label: 'Anthropic',
    url: 'https://api.anthropic.com/v1/',
  },
  { value: 'openAi', label: 'OpenAI', url: 'https://api.openai.com/v1/' },
  {
    value: 'openRouter',
    label: 'OpenRouter',
    url: 'https://openrouter.ai/api/v1/',
  },
  {
    value: 'gemini',
    label: 'Google Gemini',
    url: 'https://generativelanguage.googleapis.com/v1beta/',
  },
  { value: 'compatible', label: 'OpenAI-compatible', url: '' },
  { value: 'ollama', label: 'Ollama', url: 'http://127.0.0.1:11434/' },
  {
    value: 'localCompatible',
    label: 'Local / LM Studio',
    url: 'http://127.0.0.1:1234/v1/',
  },
];
const defaults: Required<AiSettings> = {
  enabled: false,
  providers: [],
  defaultProvider: '',
  defaultModel: '',
};
/** Local model endpoints do not require a dummy key. */
export const isLocalProvider = (kind: ProviderKind): boolean =>
  kind === 'ollama' || kind === 'localCompatible';

function provider(value: unknown): ProviderConfig | null {
  if (!value || typeof value !== 'object') return null;
  const fields = value as Record<string, unknown>;
  const { id, name, kind, baseUrl, secretRef } = fields;
  if (
    typeof id !== 'string' ||
    !/^[a-zA-Z0-9-]{1,64}$/.test(id) ||
    typeof name !== 'string' ||
    !name.trim() ||
    name.length > 80 ||
    typeof baseUrl !== 'string' ||
    baseUrl.length > 2048 ||
    !PROVIDER_KINDS.some((item) => item.value === kind) ||
    typeof secretRef !== 'string' ||
    (secretRef !== '' && secretRef !== `ai/${id}/api-key`)
  )
    return null;
  return {
    id,
    name,
    kind: kind as ProviderKind,
    baseUrl,
    secretRef,
    allowInsecureRemote: fields.allowInsecureRemote === true,
  };
}
/** Drop unknown fields including pasted keys; default network permission stays off. */
export function parseAiSettings(value: unknown): Required<AiSettings> {
  if (!value || typeof value !== 'object') return defaults;
  const fields = value as Record<string, unknown>;
  const providers = Array.isArray(fields.providers)
    ? fields.providers
        .slice(0, 32)
        .map(provider)
        .filter((item): item is ProviderConfig => item !== null)
    : [];
  const unique = providers.filter(
    (item, index) =>
      providers.findIndex((other) => other.id === item.id) === index,
  );
  const selected = unique.some((item) => item.id === fields.defaultProvider);
  return {
    enabled: fields.enabled === true,
    providers: unique,
    defaultProvider: selected ? String(fields.defaultProvider) : '',
    defaultModel:
      selected &&
      typeof fields.defaultModel === 'string' &&
      fields.defaultModel.length <= 256
        ? fields.defaultModel
        : '',
  };
}
/** Shared settings cache contains references, never API-key values or probe responses. */
export const aiSettings = localSetting(AI_KEY, parseAiSettings);
