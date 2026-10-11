// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import { SettingRow } from '../../appearance/InterfaceSettings';
import { aiSettings } from './aiSettings';
import { saveAiSettings } from './aiActions';
const useAiSettings = aiSettings.useValue;
/** Global network switch and explicitly selected default model. */
export function AiPreferences(props: {
  busy: boolean;
  run: (work: () => Promise<void>) => Promise<void>;
}): React.JSX.Element {
  const settings = useAiSettings();
  return (
    <>
      <SettingRow
        title="AI features off"
        description="Off blocks all provider requests and cancels active verification. Verify reads models/authentication metadata only; it never generates text or sends database context."
      >
        <input
          aria-label="AI features off"
          type="checkbox"
          checked={!settings.enabled}
          disabled={props.busy}
          onChange={(event) => {
            const enabled = !event.target.checked;
            void props.run(() => saveAiSettings({ ...settings, enabled }));
          }}
        />
      </SettingRow>
      <SettingRow
        title="Default model"
        description="Choose a provider below and enter a model ID from Verify. Model capabilities and limits stay unknown unless reported."
      >
        <TextField
          aria-label="Default model"
          value={settings.defaultModel}
          maxLength={256}
          disabled={!settings.defaultProvider || props.busy}
          onChange={(event) => {
            aiSettings.set({ ...settings, defaultModel: event.target.value });
          }}
          onBlur={() => {
            void props.run(() => saveAiSettings(aiSettings.get()));
          }}
        />
      </SettingRow>
    </>
  );
}
