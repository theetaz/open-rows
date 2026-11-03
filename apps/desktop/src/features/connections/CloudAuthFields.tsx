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
import { t } from './messages/routes';

const HINTS = {
  aws: t('usesSignedAws'),
  azure: t('usesSignedAzure'),
  google: t('usesSignedGcloud'),
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
        label={t('provider')}
        aria-label={t('cloudProvider')}
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
            label={t('region')}
            placeholder={t('usEast1')}
            value={draft.cloudAuth.region}
            {...fieldError(form.errors.cloudRegion)}
            onChange={(event) => {
              set({ region: event.target.value });
            }}
          />
          <TextField
            className={styles.row}
            label={t('awsProfile')}
            placeholder={t('defaultCredentials')}
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
          label={t('tenant')}
          placeholder={t('signedDefault')}
          value={draft.cloudAuth.tenant}
          onChange={(event) => {
            set({ tenant: event.target.value });
          }}
        />
      )}
      {provider === 'google' && (
        <TextField
          className={styles.row}
          label={t('account')}
          placeholder={t('activeGcloudAccount')}
          value={draft.cloudAuth.account}
          onChange={(event) => {
            set({ account: event.target.value });
          }}
        />
      )}
      <small className={styles.hint}>
        {t('hintsNewToken', { hint: HINTS[provider] })}
      </small>
      {advice && (
        <p className={styles.error} role="alert">
          {advice}
        </p>
      )}
    </div>
  );
}
