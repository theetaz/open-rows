// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { SelectField, TextField } from '@openrows/ui';
import { dumpTools } from '@openrows/ipc';
import type { HelperHop } from '@openrows/ipc';
import { CustomHopFields } from './CustomHopFields';
import { helperTool, newHelper } from './helperForm';
import { SwitchRow } from './SwitchRow';
import type { ConnectionForm } from './useConnectionForm';
import styles from './flow/Form.module.css';
import { t } from './messages/routes';

type HopKind = HelperHop['kind'];
/** The route a connection takes: no helper, a cloud provider hop or a zero-trust hop. */
export type RouteKind = 'direct' | 'cloud' | 'zero';

const HOPS: Record<Exclude<RouteKind, 'direct'>, readonly HopKind[]> = {
  cloud: ['ssm', 'kubectl', 'cloud'],
  zero: ['cloudflared', 'custom'],
};
const HOP_LABELS: Record<HopKind, string> = {
  ssm: t('hopAwsSsm'),
  kubectl: t('hopKubernetes'),
  cloud: t('hopCloudSql'),
  cloudflared: t('hopCloudflare'),
  custom: t('hopCustom'),
};

export function routeOf(helper: HelperHop | null): RouteKind {
  if (!helper) return 'direct';
  return HOPS.cloud.includes(helper.kind) ? 'cloud' : 'zero';
}

/** The helper fields of a route; a Cloud SQL proxy turns database TLS off. */
function setHop(form: ConnectionForm, kind: HopKind | null): void {
  form.update({
    helper: kind ? newHelper(kind) : null,
    ...(kind === 'cloud' ? { tlsMode: 'disable' } : {}),
  });
}

/** Choose a route card: direct clears the helper, a hop route starts at its first hop. */
export function chooseRoute(form: ConnectionForm, route: RouteKind): void {
  if (route === routeOf(form.draft.helper)) return;
  setHop(form, route === 'direct' ? null : (HOPS[route][0] ?? null));
}

/** Rows of a cloud or zero-trust route: the hop, its settings and tool advice. */
export function HelperFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element | null {
  const helper = form.draft.helper;
  const warning = useToolWarning(helper);
  if (!helper) return null;
  const route = routeOf(helper);
  return (
    <>
      <SelectField
        className={styles.row}
        label={t('hop')}
        value={helper.kind}
        options={(route === 'direct' ? [] : HOPS[route]).map((kind) => ({
          value: kind,
          label: HOP_LABELS[kind],
        }))}
        onValueChange={(kind) => {
          setHop(form, kind);
        }}
      />
      <HopFields form={form} helper={helper} />
      {warning && (
        <p className={styles.error} role="alert">
          {warning}
        </p>
      )}
      {form.errors.helper && (
        <p className={styles.error} role="alert">
          {form.errors.helper}
        </p>
      )}
      {helper.kind === 'cloud' && (
        <p className={styles.hint}>{t('authProxyEncrypts')}</p>
      )}
      <p className={styles.hint}>{t('credentialsStayTool')}</p>
    </>
  );
}

function useToolWarning(helper: HelperHop | null): string | null {
  const [warning, setWarning] = useState<string | null>(null);
  const tool = helper ? helperTool(helper) : null;
  useEffect(() => {
    let live = true;
    if (!tool) return;
    void dumpTools().then(
      (tools) => {
        const required = tools.filter(
          (found) =>
            found.tool === tool ||
            (tool === 'aws' && found.tool === 'sessionManagerPlugin'),
        );
        if (live)
          setWarning(required.find((found) => found.error)?.error ?? null);
      },
      () => {
        if (live) setWarning(t('couldNotCheck'));
      },
    );
    return () => {
      live = false;
    };
  }, [tool]);
  return tool ? warning : null;
}

type TextBinding = {
  label: string;
  value: string;
  change: (value: string) => void;
};
function HopFields({
  form,
  helper,
}: {
  form: ConnectionForm;
  helper: HelperHop;
}): React.JSX.Element {
  const set = (helper: HelperHop): void => {
    form.update({ helper });
  };
  if (helper.kind === 'custom')
    return <CustomHopFields helper={helper} set={set} />;
  const fields = textFields(helper, set);
  return (
    <>
      {fields.map((field) => (
        <TextField
          key={field.label}
          className={`${styles.row ?? ''} ${styles.mono ?? ''}`}
          label={field.label}
          value={field.value}
          spellCheck={false}
          onChange={(event) => {
            field.change(event.target.value);
          }}
        />
      ))}
      {helper.kind === 'cloud' && <CloudOptions helper={helper} set={set} />}
    </>
  );
}
function textFields(
  helper: HelperHop,
  set: (helper: HelperHop) => void,
): TextBinding[] {
  switch (helper.kind) {
    case 'custom':
      return [];
    case 'ssm':
      return ssmFields(helper, set);
    case 'kubectl':
      return kubeFields(helper, set);
    case 'cloud':
      return [
        {
          label: t('instanceConnectionName'),
          value: helper.instance,
          change: (instance) => {
            set({ ...helper, instance });
          },
        },
        {
          label: t('credentialsFileOptional'),
          value: helper.credentialsFile ?? '',
          change: (value) => {
            set({ ...helper, credentialsFile: value || null });
          },
        },
      ];
    case 'cloudflared':
      return [
        {
          label: t('accessHostname'),
          value: helper.hostname,
          change: (hostname) => {
            set({ ...helper, hostname });
          },
        },
      ];
  }
}
function ssmFields(
  helper: Extract<HelperHop, { kind: 'ssm' }>,
  set: (helper: HelperHop) => void,
): TextBinding[] {
  const text = (
    label: string,
    key: 'target' | 'remoteHost' | 'profile' | 'region',
  ): TextBinding => ({
    label,
    value: helper[key] ?? '',
    change: (value) => {
      set({ ...helper, [key]: key === 'target' ? value : value || null });
    },
  });
  return [
    text(t('ssmTarget'), 'target'),
    text(t('remoteHostOptional'), 'remoteHost'),
    remotePort(helper, set),
    text(t('awsProfileOptional'), 'profile'),
    text(t('awsRegionOptional'), 'region'),
  ];
}
function kubeFields(
  helper: Extract<HelperHop, { kind: 'kubectl' }>,
  set: (helper: HelperHop) => void,
): TextBinding[] {
  const text = (
    label: string,
    key: 'context' | 'namespace' | 'resource' | 'kubeconfig',
  ): TextBinding => ({
    label,
    value: helper[key] ?? '',
    change: (value) => {
      set({ ...helper, [key]: key === 'kubeconfig' ? value || null : value });
    },
  });
  return [
    text(t('context'), 'context'),
    text(t('namespace'), 'namespace'),
    text(t('resourcePodName'), 'resource'),
    remotePort(helper, set),
    text(t('kubeconfigFileOptional'), 'kubeconfig'),
  ];
}
function remotePort(
  helper: Extract<HelperHop, { kind: 'ssm' | 'kubectl' }>,
  set: (helper: HelperHop) => void,
): TextBinding {
  return {
    label: t('remotePort'),
    value: String(helper.remotePort),
    change: (value) => {
      set({ ...helper, remotePort: Number(value) });
    },
  };
}
function CloudOptions({
  helper,
  set,
}: {
  helper: Extract<HelperHop, { kind: 'cloud' }>;
  set: (helper: HelperHop) => void;
}): React.JSX.Element {
  return (
    <>
      <SelectField
        className={styles.row}
        label={t('proxyFlavour')}
        value={helper.flavor}
        options={[
          { value: 'cloudsql', label: t('cloudSql') },
          { value: 'alloydb', label: t('alloydb') },
        ]}
        onValueChange={(flavor) => {
          set({ ...helper, flavor });
        }}
      />
      <SwitchRow
        label={t('iamAuthentication')}
        checked={helper.iamAuth}
        onChange={(iamAuth) => {
          set({ ...helper, iamAuth });
        }}
      />
      <SwitchRow
        label={t('privateIp')}
        checked={helper.privateIp}
        onChange={(privateIp) => {
          set({ ...helper, privateIp });
        }}
      />
    </>
  );
}
