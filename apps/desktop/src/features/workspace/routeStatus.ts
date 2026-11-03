// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';
import { engineName } from '../connections/engines';
import { t } from './messages';

/** The complete configured route shown in the connection status popover. */
export function routeStatus(profile: ProfileDto): string {
  const route: string[] = [];
  const helper = profile.helper;
  if (helper) {
    switch (helper.kind) {
      case 'ssm':
        route.push(t('awsSsmTarget', { target: helper.target }));
        break;
      case 'kubectl':
        route.push(t('kubectlPortForward', { resource: helper.resource }));
        break;
      case 'cloud':
        route.push(
          helper.flavor === 'cloudsql'
            ? t('cloudSqlAuth', { instance: helper.instance })
            : t('alloydbAuthProxy', { instance: helper.instance }),
        );
        break;
      case 'cloudflared':
        route.push(`cloudflared ${helper.hostname}`);
        break;
    }
  }
  if (profile.proxy)
    route.push(
      `${profile.proxy.kind === 'socks5' ? 'SOCKS5' : t('httpConnect')} ${profile.proxy.host}`,
    );
  if (profile.ssh)
    route.push(...profile.ssh.jumpHosts, `SSH ${profile.ssh.host}`);
  if (!route.length) route.push(t('direct'));
  if (profile.tlsMode !== 'disable') route.push('TLS');
  route.push(engineName(profile.engine));
  return t('routeRoute', { route: route.join(' → ') });
}
