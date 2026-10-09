// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { profile } from '../../test/ipcStub';
import { routeStatus } from './routeStatus';
it('shows the cloud hop before TLS and the database in the status route', () => {
  expect(
    routeStatus(
      profile('routed', {
        helper: {
          kind: 'kubectl',
          context: 'test',
          namespace: 'default',
          resource: 'svc/db',
          remotePort: 5432,
          kubeconfig: null,
        },
        tlsMode: 'verify-full',
      }),
    ),
  ).toBe('Route: kubectl port-forward svc/db → TLS → PostgreSQL');
});
