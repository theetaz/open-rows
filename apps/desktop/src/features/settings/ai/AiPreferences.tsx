// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import { SettingRow } from '../../appearance/InterfaceSettings';
import { aiSettings, CLI_TURN } from './aiSettings';
import { saveAiSettings } from './aiActions';
import { t } from '../messages/lazyAi';
const useAiSettings = aiSettings.useValue;
/** Global network switch and explicitly selected default model. */
export function AiPreferences(props: {
  busy: boolean;
  run: (work: () => Promise<void>) => Promise<void>;
}): React.JSX.Element {
  const settings = useAiSettings();
  return (
    <>
      <SettingRow title={t('aiFeaturesOff')} description={t('offBlocksAll')}>
        <input
          aria-label={t('aiFeaturesOff')}
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
        title={t('defaultModel')}
        description={t('chooseProviderBelow')}
      >
        <TextField
          aria-label={t('defaultModel')}
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
      <SettingRow
        title={t('codexTurnDeadline')}
        description={t('secondsOneCodex')}
      >
        <TextField
          aria-label={t('codexTurnDeadlineSeconds')}
          type="number"
          min={CLI_TURN.min}
          max={CLI_TURN.max}
          step={30}
          defaultValue={settings.cliTurnSeconds}
          key={settings.cliTurnSeconds}
          disabled={props.busy}
          onBlur={(event) => {
            const seconds = Math.round(Number(event.target.value));
            const cliTurnSeconds = Math.min(
              CLI_TURN.max,
              Math.max(CLI_TURN.min, seconds || CLI_TURN.initial),
            );
            event.target.value = String(cliTurnSeconds);
            if (cliTurnSeconds === settings.cliTurnSeconds) return;
            void props.run(() =>
              saveAiSettings({ ...aiSettings.get(), cliTurnSeconds }),
            );
          }}
        />
      </SettingRow>
    </>
  );
}
