// SPDX-License-Identifier: Apache-2.0
import { deleteAiKey, promptAiKey, writeSettingsFile } from '@openrows/ipc';
import type { AiSettings, ProviderConfig } from '@openrows/ipc';
import { settingsText } from '../settingsFile';
import { aiSettings, parseAiSettings } from './aiSettings';
/** Persist before invoking native operations; rollback preferences on write failure. */
export async function saveAiSettings(next: AiSettings): Promise<void> {
  const previous = aiSettings.get();
  aiSettings.set(parseAiSettings(next));
  try {
    await writeSettingsFile(settingsText());
  } catch {
    aiSettings.set(previous);
    throw new Error('AI settings could not be saved.');
  }
}
/** Save configuration, then optionally open the native key dialog for it. */
export async function saveProvider(
  config: ProviderConfig,
  askKey: boolean,
): Promise<void> {
  const settings = aiSettings.get();
  const providers = settings.providers.filter((item) => item.id !== config.id);
  await saveAiSettings({ ...settings, providers: [...providers, config] });
  if (askKey) {
    try {
      await promptAiKey(config.id);
    } catch {
      throw new Error(
        'Provider saved, but the API key could not be stored. Use Set API key to retry.',
      );
    }
  }
}
/** Delete the key first; a failed keychain operation keeps the provider editable. */
export async function removeProvider(config: ProviderConfig): Promise<void> {
  try {
    await deleteAiKey(config.id);
  } catch {
    throw new Error('The API key could not be removed. Provider retained.');
  }
  const settings = aiSettings.get();
  const selected = settings.defaultProvider === config.id;
  await saveAiSettings({
    ...settings,
    providers: settings.providers.filter((item) => item.id !== config.id),
    defaultProvider: selected ? '' : settings.defaultProvider,
    defaultModel: selected ? '' : settings.defaultModel,
  });
}
