// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import {
  cloudProviders,
  effectiveProvider,
  insecureRouteAdvice,
} from './cloudAuthForm';
import type { CloudAuthDraft } from './cloudAuthForm';
import { fieldError } from './form';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

const HINTS = {
  aws: 'Uses your signed-in AWS CLI (aws rds generate-db-auth-token). Enable IAM on the database user.',
  azure:
    'Uses your signed-in Azure CLI (az account get-access-token). Use the Entra user or group as the database user.',
  google:
    'Uses your signed-in gcloud (gcloud sql generate-login-token). Use the IAM user as the database user.',
} as const;

/** Where a cloud token comes from; the token itself is never shown or saved. */
export function CloudAuthFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft } = form;
  const provider = effectiveProvider(draft);
  const set = (change: Partial<CloudAuthDraft>): void => {
    form.update({ cloudAuth: { ...draft.cloudAuth, ...change } });
  };
  const advice = insecureRouteAdvice(draft);
  return (
    <div className={styles.stack}>
      <SelectField
        className={styles.row}
        label="Provider"
        aria-label="Cloud provider"
        value={provider}
        options={cloudProviders(draft.engine)}
        onValueChange={(next) => {
          set({ provider: next });
        }}
      />
      {provider === 'aws' && (
        <>
          <TextField
            className={styles.row}
            label="Region"
            placeholder="us-east-1"
            value={draft.cloudAuth.region}
            {...fieldError(form.errors.cloudRegion)}
            onChange={(event) => {
              set({ region: event.target.value });
            }}
          />
          <TextField
            className={styles.row}
            label="AWS profile"
            placeholder="Default credentials"
            value={draft.cloudAuth.awsProfile}
            onChange={(event) => {
              set({ awsProfile: event.target.value });
            }}
          />
        </>
      )}
      {provider === 'azure' && (
        <TextField
          className={styles.row}
          label="Tenant"
          placeholder="Signed-in default"
          value={draft.cloudAuth.tenant}
          onChange={(event) => {
            set({ tenant: event.target.value });
          }}
        />
      )}
      {provider === 'google' && (
        <TextField
          className={styles.row}
          label="Account"
          placeholder="Active gcloud account"
          value={draft.cloudAuth.account}
          onChange={(event) => {
            set({ account: event.target.value });
          }}
        />
      )}
      <small className={styles.hint}>
        {HINTS[provider]} A new token is fetched for every connection and never
        saved.
      </small>
      {advice && (
        <p className={styles.error} role="alert">
          {advice}
        </p>
      )}
    </div>
  );
}
