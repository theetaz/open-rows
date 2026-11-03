// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import {
  DisclosureSection,
  SelectField,
  TextField,
  Switch,
} from '@openrows/ui';
import { dumpTools } from '@openrows/ipc';
import type { HelperHop } from '@openrows/ipc';
import { CustomHopFields } from './CustomHopFields';
import { helperTool, newHelper } from './helperForm';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/routes';

type Choice = 'off' | HelperHop['kind'];
const choices: readonly { value: Choice; label: string }[] = [
  { value: 'custom', label: t('cloudZeroTrust') },
  { value: 'off', label: t('directSshProxy') },
  { value: 'ssm', label: t('cloudZeroTrustAws') },
  { value: 'kubectl', label: t('cloudZeroTrustKubernetes') },
  { value: 'cloud', label: t('cloudZeroTrustCloud') },
  { value: 'cloudflared', label: t('cloudZeroTrustCloudflare') },
];

/** Cloud route settings, including locally trusted command templates. */
export function HelperFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const helper = form.draft.helper;
  const warning = useToolWarning(helper);
  return (
    <DisclosureSection
      label={t('routeCloudZero')}
      defaultOpen={Boolean(helper)}
      forceOpen={Boolean(form.errors.helper)}
    >
      <div className={styles.section}>
        <SelectField
          label={t('route')}
          value={helper?.kind ?? 'off'}
          options={choices}
          onValueChange={(kind) => {
            const helper = kind === 'off' ? null : newHelper(kind);
            form.update({
              helper,
              ...(kind === 'cloud' ? { tlsMode: 'disable' } : {}),
            });
          }}
        />
        {helper && <HopFields form={form} helper={helper} />}
        {warning && <p role="alert">{warning}</p>}
        {form.errors.helper && <p role="alert">{form.errors.helper}</p>}
        {helper?.kind === 'cloud' && <p>{t('authProxyEncrypts')}</p>}
        {helper && <p>{t('credentialsStayTool')}</p>}
      </div>
    </DisclosureSection>
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
          className={styles.row}
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
      <label className={styles.switchRow}>
        <Switch
          aria-label={t('iamAuthentication')}
          checked={helper.iamAuth}
          onCheckedChange={(iamAuth) => {
            set({ ...helper, iamAuth });
          }}
        />
        {t('iamAuthentication')}
      </label>
      <label className={styles.switchRow}>
        <Switch
          aria-label={t('privateIp')}
          checked={helper.privateIp}
          onCheckedChange={(privateIp) => {
            set({ ...helper, privateIp });
          }}
        />
        {t('privateIp')}
      </label>
    </>
  );
}
