// SPDX-License-Identifier: Apache-2.0
import { Button, Icon } from '@openrows/ui';
import type { ServerStatus } from '@openrows/ipc';
import styles from './Ai.module.css';
import { rich, t } from '../messages/lazyAi';
/** Scrubbed token/client metadata with revoke and disconnect controls. */
export function McpClients(props: {
  status: ServerStatus;
  busy: boolean;
  revoke: (id: string) => void;
  disconnect: (id: string) => void;
}): React.JSX.Element {
  return (
    <>
      <div>
        <h4 className={styles.title}>{t('readOnlyTokens')}</h4>
        {props.status.tokens.length > 0 && (
          <div className={[styles.group, styles.withIcons].join(' ')}>
            {props.status.tokens.map((token) => (
              <div key={token.id} className={styles.row}>
                <span className={styles.rowIcon}>
                  <Icon name="key" />
                </span>
                <span className={styles.rowText}>
                  {rich('idCountConnections', {
                    id: (
                      <code className={styles.mono}>
                        {token.id.slice(0, 12)}
                      </code>
                    ),
                    count: token.scope.connections.length,
                    value: new Date(
                      token.scope.expires * 1000,
                    ).toLocaleString(),
                  })}
                </span>
                <Button
                  size="sm"
                  variant="destructive"
                  disabled={props.busy}
                  onClick={() => {
                    props.revoke(token.id);
                  }}
                >
                  {t('revokeToken')}
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
      <Clients {...props} />
    </>
  );
}

function Clients(props: {
  status: ServerStatus;
  busy: boolean;
  disconnect: (id: string) => void;
}): React.JSX.Element {
  return (
    <div>
      <h4 className={styles.title}>{t('connectedClients')}</h4>
      {props.status.clients.length > 0 && (
        <div className={[styles.group, styles.withIcons].join(' ')}>
          {props.status.clients.map((client) => (
            <div key={client.id} className={styles.row}>
              <span className={styles.rowIcon}>
                <Icon name="activity" />
              </span>
              <span className={styles.rowText}>
                {rich('idActiveActive', {
                  id: (
                    <code className={styles.mono}>
                      {client.id.slice(0, 12)}
                    </code>
                  ),
                  active: client.active,
                })}
              </span>
              <Button
                size="sm"
                disabled={props.busy}
                onClick={() => {
                  props.disconnect(client.id);
                }}
              >
                {t('disconnectClient')}
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
