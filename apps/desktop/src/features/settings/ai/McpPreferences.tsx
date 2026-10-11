// SPDX-License-Identifier: Apache-2.0
import type { McpSettings } from '@openrows/ipc';
import { TextField } from '@openrows/ui';
/** Explicit local bridge/HTTP choices and OS-selected port by default. */
export function McpPreferences(props: {
  settings: Required<McpSettings>;
  busy: boolean;
  change: (settings: Required<McpSettings>) => void;
}): React.JSX.Element {
  const { settings, busy, change } = props;
  return (
    <fieldset disabled={busy}>
      <legend>MCP server</legend>
      <label>
        <input
          type="checkbox"
          checked={settings.enabled}
          onChange={(event) => {
            change({ ...settings, enabled: event.target.checked });
          }}
        />{' '}
        Enable MCP stdio bridge
      </label>
      <label>
        <input
          type="checkbox"
          checked={settings.httpEnabled}
          onChange={(event) => {
            change({ ...settings, httpEnabled: event.target.checked });
          }}
        />{' '}
        Allow direct localhost HTTP
      </label>
      <TextField
        label="MCP port (0 = automatic)"
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
      <p>
        AI features must be enabled. Only open databases permitted by their
        data-sharing policy are exposed. Reads are bounded; SQL proposals open
        for user review.
      </p>
    </fieldset>
  );
}
