// SPDX-License-Identifier: Apache-2.0
import type { HelperHop } from '@openrows/ipc';
import { t } from './messages/routes';

/** A new structured route; cloud proxies encrypt their upstream connection. */
export function newHelper(kind: HelperHop['kind']): HelperHop {
  switch (kind) {
    case 'custom':
      return {
        kind,
        program: '',
        args: ['{local_port}'],
        readyTimeoutSeconds: null,
      };
    case 'ssm':
      return {
        kind,
        target: '',
        remoteHost: null,
        remotePort: 5432,
        profile: null,
        region: null,
      };
    case 'kubectl':
      return {
        kind,
        context: '',
        namespace: 'default',
        resource: '',
        remotePort: 5432,
        kubeconfig: null,
      };
    case 'cloud':
      return {
        kind,
        instance: '',
        flavor: 'cloudsql',
        iamAuth: false,
        privateIp: true,
        credentialsFile: null,
      };
    case 'cloudflared':
      return { kind, hostname: '' };
  }
}

/** Validate fields and unsupported combinations before saving or testing. */
export function helperError(
  helper: HelperHop | null,
  chained: boolean,
): string | undefined {
  if (!helper) return undefined;
  if (chained) return t('cloudRoutesCannot');
  if (helper.kind === 'custom') return customError(helper);
  const values = Object.values(helper);
  if (
    values.some(
      (value) =>
        typeof value === 'string' &&
        (value.startsWith('-') ||
          Array.from(value).some((char) => char.charCodeAt(0) < 32)),
    )
  )
    return t('routeValuesCannot');
  return requiredError(helper);
}

function requiredError(
  helper: Exclude<HelperHop, { kind: 'custom' }>,
): string | undefined {
  switch (helper.kind) {
    case 'ssm':
      return helper.target.trim() && validPort(helper.remotePort)
        ? undefined
        : t('targetRemotePort');
    case 'kubectl':
      return helper.context.trim() &&
        helper.namespace.trim() &&
        /^(pod|svc|deployment)\/.+$/.test(helper.resource) &&
        validPort(helper.remotePort)
        ? undefined
        : t('contextNamespaceResource');
    case 'cloud':
      return helper.instance.trim() && !/[?&=#\s]/.test(helper.instance)
        ? undefined
        : t('enterInstanceConnection');
    case 'cloudflared':
      return helper.hostname.trim() && !/[/?@#:\s]/.test(helper.hostname)
        ? undefined
        : t('enterAccessHostname');
  }
}
function customError(
  helper: Extract<HelperHop, { kind: 'custom' }>,
): string | undefined {
  if (
    !/^(\/|[A-Za-z]:[\\/])/.test(helper.program) ||
    helper.program.includes('\0')
  )
    return t('programMustAbsolute');
  if (!helper.args.some((arg) => arg.includes('{local_port}')))
    return t('includeLocalPort', { local_port: '{local_port}' });
  if (
    helper.args.some(
      (arg) =>
        arg.includes('\0') ||
        /\{(?!local_port\}|remote_host\}|remote_port\})/.test(arg),
    )
  )
    return t('useOnlyLocal', {
      local_port: '{local_port}',
      remote_host: '{remote_host}',
      remote_port: '{remote_port}',
    });
  const timeout = helper.readyTimeoutSeconds;
  if (
    timeout !== null &&
    timeout !== undefined &&
    (!Number.isInteger(timeout) || timeout < 1 || timeout > 300)
  )
    return t('readinessTimeoutMust');
  return undefined;
}

function validPort(port: number): boolean {
  return Number.isInteger(port) && port > 0 && port <= 65535;
}

/** The required tool key in Settings ▸ Tools. */
export function helperTool(helper: HelperHop): string {
  switch (helper.kind) {
    case 'custom':
      return '';
    case 'ssm':
      return 'aws';
    case 'kubectl':
      return 'kubectl';
    case 'cloud':
      return helper.flavor === 'cloudsql'
        ? 'cloudSqlProxy'
        : 'alloydbAuthProxy';
    case 'cloudflared':
      return 'cloudflared';
  }
}
