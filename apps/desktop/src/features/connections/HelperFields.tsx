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

type Choice = 'off' | HelperHop['kind'];
const choices: readonly { value: Choice; label: string }[] = [
  { value: 'custom', label: 'Cloud / zero-trust · Custom command' },
  { value: 'off', label: 'Direct / SSH / proxy' },
  { value: 'ssm', label: 'Cloud / zero-trust · AWS SSM' },
  { value: 'kubectl', label: 'Cloud / zero-trust · Kubernetes' },
  { value: 'cloud', label: 'Cloud / zero-trust · Cloud SQL / AlloyDB' },
  { value: 'cloudflared', label: 'Cloud / zero-trust · Cloudflare Access' },
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
      label="Route · Cloud / zero-trust"
      defaultOpen={Boolean(helper)}
      forceOpen={Boolean(form.errors.helper)}
    >
      <div className={styles.section}>
        <SelectField
          label="Route"
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
        {helper?.kind === 'cloud' && (
          <p>
            The auth proxy encrypts traffic to Google. Database TLS defaults to
            Off; verify modes use the original database host.
          </p>
        )}
        {helper && (
          <p>
            Credentials stay in the tool’s own configuration. Cloud routes
            cannot combine with SSH or a proxy.
          </p>
        )}
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
        if (live) setWarning('Could not check tools. Check Settings ▸ Tools.');
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
          label: 'Instance connection name / URI',
          value: helper.instance,
          change: (instance) => {
            set({ ...helper, instance });
          },
        },
        {
          label: 'Credentials file (optional)',
          value: helper.credentialsFile ?? '',
          change: (value) => {
            set({ ...helper, credentialsFile: value || null });
          },
        },
      ];
    case 'cloudflared':
      return [
        {
          label: 'Access hostname',
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
    text('SSM target', 'target'),
    text('Remote host (optional)', 'remoteHost'),
    remotePort(helper, set),
    text('AWS profile (optional)', 'profile'),
    text('AWS region (optional)', 'region'),
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
    text('Context', 'context'),
    text('Namespace', 'namespace'),
    text('Resource (pod/name, svc/name, deployment/name)', 'resource'),
    remotePort(helper, set),
    text('Kubeconfig file (optional)', 'kubeconfig'),
  ];
}
function remotePort(
  helper: Extract<HelperHop, { kind: 'ssm' | 'kubectl' }>,
  set: (helper: HelperHop) => void,
): TextBinding {
  return {
    label: 'Remote port',
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
        label="Proxy flavour"
        value={helper.flavor}
        options={[
          { value: 'cloudsql', label: 'Cloud SQL' },
          { value: 'alloydb', label: 'AlloyDB' },
        ]}
        onValueChange={(flavor) => {
          set({ ...helper, flavor });
        }}
      />
      <label className={styles.switchRow}>
        <Switch
          aria-label="IAM authentication"
          checked={helper.iamAuth}
          onCheckedChange={(iamAuth) => {
            set({ ...helper, iamAuth });
          }}
        />
        IAM authentication
      </label>
      <label className={styles.switchRow}>
        <Switch
          aria-label="Private IP"
          checked={helper.privateIp}
          onCheckedChange={(privateIp) => {
            set({ ...helper, privateIp });
          }}
        />
        Private IP
      </label>
    </>
  );
}
