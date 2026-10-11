// SPDX-License-Identifier: Apache-2.0
import { Switch, TextField } from '@openrows/ui';
import { historySettings, parseHistorySettings } from './historySettings';
/** Settings ▸ General ▸ History; saved changes sweep stored history. */
export function HistorySection(): React.JSX.Element {
  const settings = historySettings.useValue();
  return (
    <section aria-label="History retention">
      <h3>History</h3>
      <label>
        Keep forever{' '}
        <Switch
          aria-label="Keep history forever"
          checked={settings.keepForever}
          onCheckedChange={(keepForever) => {
            historySettings.set({ ...settings, keepForever });
          }}
        />
      </label>
      <TextField
        label="Maximum entries per connection"
        type="number"
        min={1}
        max={4294967295}
        disabled={settings.keepForever}
        value={String(settings.maxEntries)}
        onChange={(event) => {
          historySettings.set(
            parseHistorySettings({
              ...settings,
              maxEntries: Number(event.target.value),
            }),
          );
        }}
      />
      <TextField
        label="Maximum age in days"
        type="number"
        min={1}
        max={4294967295}
        disabled={settings.keepForever}
        value={String(settings.maxAgeDays)}
        onChange={(event) => {
          historySettings.set(
            parseHistorySettings({
              ...settings,
              maxAgeDays: Number(event.target.value),
            }),
          );
        }}
      />
      <p>
        Expired entries are removed on write and startup. Lowering a limit
        removes matching history.
      </p>
    </section>
  );
}
