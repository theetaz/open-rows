// SPDX-License-Identifier: Apache-2.0
import type { McpSettings } from '@openrows/ipc';
import { TextField } from '@openrows/ui';
import { t } from '../messages/lazyAi';
/** Explicit local bridge/HTTP choices and OS-selected port by default. */
export function McpPreferences(props: {
  settings: Required<McpSettings>;
  busy: boolean;
  change: (settings: Required<McpSettings>) => void;
}): React.JSX.Element {
  const { settings, busy, change } = props;
  return (
    <fieldset disabled={busy}>
      <legend>{t('mcpServer')}</legend>
      <label>
        <input
          type="checkbox"
          checked={settings.enabled}
          onChange={(event) => {
            change({ ...settings, enabled: event.target.checked });
          }}
        />{' '}
        {t('enableMcpStdio')}
      </label>
      <label>
        <input
          type="checkbox"
          checked={settings.httpEnabled}
          onChange={(event) => {
            change({ ...settings, httpEnabled: event.target.checked });
          }}
        />{' '}
        {t('allowDirectLocalhost')}
      </label>
      <TextField
        label={t('mcpPort0')}
        type="number"
        min={0}
        max={65535}
        value={String(settings.port)}
        onChange={(event) => {
          const port = Number(event.target.value);
          if (Number.isInteger(port) && port >= 0 && port <= 65535)
            change({ ...settings, port });
        }}
      />
      <p>{t('aiFeaturesMust')}</p>
    </fieldset>
  );
}
