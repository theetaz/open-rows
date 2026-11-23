// SPDX-License-Identifier: Apache-2.0
import { Button, Icon } from '@openrows/ui';
import styles from './Ai.module.css';
import {
  createMcpToken,
  disconnectMcpClient,
  revokeMcpToken,
} from '@openrows/ipc';
import type { ServerStatus } from '@openrows/ipc';
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
    <section aria-label={t('mcpSettings')} className={styles.stack}>
      <McpPreferences
        settings={settings}
        busy={busy}
        change={(next) => {
          void run(() => saveMcpSettings(next));
        }}
        status={
          <StatusRow
            status={status}
            busy={busy}
            refresh={() => void run(async () => {})}
          />
        }
      />
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
        <div className={styles.secret}>
          <p>{t('copyTokenNow')}</p>
          <output aria-label={t('newMcpToken')}>{value}</output>
          <Button
            size="sm"
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

/** Where the server listens, with an explicit refresh. */
function StatusRow(props: {
  status: ServerStatus | null;
  busy: boolean;
  refresh: () => void;
}): React.JSX.Element {
  const { status } = props;
  return (
    <div className={styles.row}>
      <span
        className={styles.rowIcon}
        data-tone={status?.port ? 'success' : undefined}
      >
        <Icon name={status?.port ? 'check' : 'minus'} />
      </span>
      <p role="status" className={styles.rowBody}>
        {status?.port
          ? status.httpEnabled
            ? t('listening1270', { port: String(status.port) })
            : t('listening12700', { port: String(status.port) })
          : t('mcpServerStopped')}
      </p>
      <Button size="sm" disabled={props.busy} onClick={props.refresh}>
        {t('refreshMcpStatus')}
      </Button>
    </div>
  );
}
