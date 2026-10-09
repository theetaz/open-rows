// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';
import { engineName } from '../connections/engines';

/** The complete configured route shown in the connection status popover. */
export function routeStatus(profile: ProfileDto): string {
  const route: string[] = [];
  const helper = profile.helper;
  if (helper) {
    switch (helper.kind) {
      case 'ssm':
        route.push(`AWS SSM ${helper.target}`);
        break;
      case 'kubectl':
        route.push(`kubectl port-forward ${helper.resource}`);
        break;
      case 'cloud':
        route.push(
          `${helper.flavor === 'cloudsql' ? 'Cloud SQL' : 'AlloyDB'} auth proxy ${helper.instance}`,
        );
        break;
      case 'cloudflared':
        route.push(`cloudflared ${helper.hostname}`);
        break;
    }
  }
  if (profile.proxy)
    route.push(
      `${profile.proxy.kind === 'socks5' ? 'SOCKS5' : 'HTTP CONNECT'} ${profile.proxy.host}`,
    );
  if (profile.ssh)
    route.push(...profile.ssh.jumpHosts, `SSH ${profile.ssh.host}`);
  if (!route.length) route.push('Direct');
  if (profile.tlsMode !== 'disable') route.push('TLS');
  route.push(engineName(profile.engine));
  return `Route: ${route.join(' → ')}`;
}
