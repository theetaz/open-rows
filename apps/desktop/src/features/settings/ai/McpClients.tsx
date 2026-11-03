// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { ServerStatus } from '@openrows/ipc';
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
      <h4>{t('readOnlyTokens')}</h4>
      {props.status.tokens.map((token) => (
        <div key={token.id}>
          {rich('idCountConnections', {
            id: <code>{token.id.slice(0, 12)}</code>,
            count: token.scope.connections.length,
            value: new Date(token.scope.expires * 1000).toLocaleString(),
          })}{' '}
          <Button
            disabled={props.busy}
            onClick={() => {
              props.revoke(token.id);
            }}
          >
            {t('revokeToken')}
          </Button>
        </div>
      ))}
      <h4>{t('connectedClients')}</h4>
      {props.status.clients.map((client) => (
        <div key={client.id}>
          {rich('idActiveActive', {
            id: <code>{client.id.slice(0, 12)}</code>,
            active: client.active,
          })}{' '}
          <Button
            disabled={props.busy}
            onClick={() => {
              props.disconnect(client.id);
            }}
          >
            {t('disconnectClient')}
          </Button>
        </div>
      ))}
    </>
  );
}
