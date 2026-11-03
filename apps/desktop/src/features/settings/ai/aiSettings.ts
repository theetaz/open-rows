// SPDX-License-Identifier: Apache-2.0
import type { AiSettings, ProviderConfig, ProviderKind } from '@openrows/ipc';
import { localSetting } from '../localSetting';
import { t } from '../messages/validation';
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
    label: t('anthropic'),
    url: 'https://api.anthropic.com/v1/',
  },
  { value: 'openAi', label: t('openai'), url: 'https://api.openai.com/v1/' },
  {
    value: 'openRouter',
    label: t('openrouter'),
    url: 'https://openrouter.ai/api/v1/',
  },
  {
    value: 'gemini',
    label: t('googleGemini'),
    url: 'https://generativelanguage.googleapis.com/v1beta/',
  },
  { value: 'compatible', label: t('openaiCompatible'), url: '' },
  { value: 'ollama', label: t('ollama'), url: 'http://127.0.0.1:11434/' },
  {
    value: 'localCompatible',
    label: t('localLmStudio'),
    url: 'http://127.0.0.1:1234/v1/',
  },
  { value: 'codex', label: t('codexCliOwn'), url: '' },
];
/** Codex turn deadline bounds in seconds; the native side enforces the same. */
export const CLI_TURN = { min: 60, max: 900, initial: 300 } as const;
const defaults: Required<AiSettings> = {
  enabled: false,
  providers: [],
  defaultProvider: '',
  defaultModel: '',
  cliTurnSeconds: CLI_TURN.initial,
};
function cliTurn(value: unknown): number {
  return typeof value === 'number' &&
    Number.isInteger(value) &&
    value >= CLI_TURN.min &&
    value <= CLI_TURN.max
    ? value
    : CLI_TURN.initial;
}
/** Local model endpoints do not require a dummy key. */
export const isLocalProvider = (kind: ProviderKind): boolean =>
  kind === 'ollama' || kind === 'localCompatible';

/** Installed CLIs own their sign-in: no API key or endpoint. */
export const isCliProvider = (kind: ProviderKind): boolean => kind === 'codex';

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
    cliTurnSeconds: cliTurn(fields.cliTurnSeconds),
  };
}
/** Shared settings cache contains references, never API-key values or probe responses. */
export const aiSettings = localSetting(AI_KEY, parseAiSettings);
