// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import {
  createMcpToken,
  disconnectMcpClient,
  revokeMcpToken,
} from '@openrows/ipc';
import { McpServers } from './McpServers';
import { McpProjectServers } from './McpProjectServers';
import { useMcpServer } from './useMcpServer';
import { AuditViewer } from './AuditViewer';
import { McpPreferences } from './McpPreferences';
import { McpTokenForm } from './McpTokenForm';
import { McpClients } from './McpClients';
import { mcpSettings, saveMcpSettings } from './mcpSettings';
import { t } from '../messages/lazyAi';
const useMcpSettings = mcpSettings.useValue;
/** Lazy MCP controls; plaintext tokens exist only in the current one-time view. */
export default function McpPane(): React.JSX.Element {
  const settings = useMcpSettings();
  const { status, value, setValue, failure, busy, run } = useMcpServer();
  return (
    <section aria-label={t('mcpSettings')}>
      <McpPreferences
        settings={settings}
        busy={busy}
        change={(next) => {
          void run(() => saveMcpSettings(next));
        }}
      />
      <p role="status">
        {status?.port
          ? status.httpEnabled
            ? t('listening1270', { port: String(status.port) })
            : t('listening12700', { port: String(status.port) })
          : t('mcpServerStopped')}
      </p>
      <Button
        disabled={busy}
        onClick={() => {
          void run(async () => {});
        }}
      >
        {t('refreshMcpStatus')}
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
          <p>{t('copyTokenNow')}</p>
          <output aria-label={t('newMcpToken')}>{value}</output>
          <Button
            onClick={() => {
              setValue('');
            }}
          >
            {t('hideToken')}
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
      <McpServers />
      <McpProjectServers />
      <AuditViewer />
    </section>
  );
}
