// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { ServerStatus } from '@openrows/ipc';
/** Scrubbed token/client metadata with revoke and disconnect controls. */
export function McpClients(props: {
  status: ServerStatus;
  busy: boolean;
  revoke: (id: string) => void;
  disconnect: (id: string) => void;
}): React.JSX.Element {
  return (
    <>
      <h4>Read-only tokens</h4>
      {props.status.tokens.map((token) => (
        <div key={token.id}>
          <code>{token.id.slice(0, 12)}</code> ·{' '}
          {token.scope.connections.length} connections · expires{' '}
          {new Date(token.scope.expires * 1000).toLocaleString()}{' '}
          <Button
            disabled={props.busy}
            onClick={() => {
              props.revoke(token.id);
            }}
          >
            Revoke token
          </Button>
        </div>
      ))}
      <h4>Connected clients</h4>
      {props.status.clients.map((client) => (
        <div key={client.id}>
          <code>{client.id.slice(0, 12)}</code> · {client.active} active
          requests{' '}
          <Button
            disabled={props.busy}
            onClick={() => {
              props.disconnect(client.id);
            }}
          >
            Disconnect client
          </Button>
        </div>
      ))}
    </>
  );
}
