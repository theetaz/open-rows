// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import {
  createMcpToken,
  disconnectMcpClient,
  revokeMcpToken,
} from '@openrows/ipc';
import { useMcpServer } from './useMcpServer';
import { AuditViewer } from './AuditViewer';
import { McpPreferences } from './McpPreferences';
import { McpTokenForm } from './McpTokenForm';
import { McpClients } from './McpClients';
import { mcpSettings, saveMcpSettings } from './mcpSettings';
const useMcpSettings = mcpSettings.useValue;
/** Lazy MCP controls; plaintext tokens exist only in the current one-time view. */
export default function McpPane(): React.JSX.Element {
  const settings = useMcpSettings();
  const { status, value, setValue, failure, busy, run } = useMcpServer();
  return (
    <section aria-label="MCP settings">
      <McpPreferences
        settings={settings}
        busy={busy}
        change={(next) => {
          void run(() => saveMcpSettings(next));
        }}
      />
      <p role="status">
        {status?.port
          ? `Listening on 127.0.0.1:${String(status.port)} · stdio bridge${status.httpEnabled ? ' and HTTP' : ''}`
          : 'MCP server stopped'}
      </p>
      <Button
        disabled={busy}
        onClick={() => {
          void run(async () => {});
        }}
      >
        Refresh MCP status
      </Button>
      <McpTokenForm
        busy={busy}
        create={(scope) => {
          setValue('');
          void run(async () => {
            const token = await createMcpToken(scope);
            setValue(token.value);
          });
        }}
      />
      {value && (
        <div>
          <p>Copy this token now. It is shown once.</p>
          <output aria-label="New MCP token">{value}</output>
          <Button
            onClick={() => {
              setValue('');
            }}
          >
            Hide token
          </Button>
        </div>
      )}
      {status && (
        <McpClients
          status={status}
          busy={busy}
          revoke={(id) => {
            setValue('');
            void run(() => revokeMcpToken(id));
          }}
          disconnect={(id) => {
            void run(() => disconnectMcpClient(id));
          }}
        />
      )}
      {failure && <p role="alert">{failure}</p>}
      <AuditViewer />
    </section>
  );
}
