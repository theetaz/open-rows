// SPDX-License-Identifier: Apache-2.0
import type { CloudAuthDto, EngineDto, ProfileDto } from '@openrows/ipc';
import type { Draft, DraftErrors } from './form';
import { t } from './messages/routes';

type CloudProvider = CloudAuthDto['provider'];

/** Non-secret cloud sign-in settings as typed; a token is never in the form. */
export type CloudAuthDraft = {
  provider: CloudProvider;
  region: string;
  awsProfile: string;
  tenant: string;
  account: string;
};

const AWS = { value: 'aws', label: t('awsRdsAuroraIam') } as const;
const AZURE = { value: 'azure', label: t('microsoftEntraId') } as const;
const GOOGLE = { value: 'google', label: t('googleCloudSql') } as const;

/** The providers that issue tokens this engine can log in with. */
export function cloudProviders(
  engine: EngineDto,
): readonly { value: CloudProvider; label: string }[] {
  switch (engine) {
    case 'postgres':
    case 'mysql':
      return [AWS, AZURE, GOOGLE];
    case 'sqlserver':
      return [AZURE];
    default:
      return [];
  }
}

export function emptyCloudAuth(): CloudAuthDraft {
  return {
    provider: 'aws',
    region: '',
    awsProfile: '',
    tenant: '',
    account: '',
  };
}

/** The provider a draft really uses: SQL Server only has Entra ID. */
export function effectiveProvider(draft: Draft): CloudProvider {
  const offered = cloudProviders(draft.engine);
  return offered.some((choice) => choice.value === draft.cloudAuth.provider)
    ? draft.cloudAuth.provider
    : (offered[0]?.value ?? draft.cloudAuth.provider);
}

const optional = (text: string): string | null => text.trim() || null;

/** Payload for save and test; switching policy clears the settings. */
export function cloudAuthFields(draft: Draft): CloudAuthDto | null {
  if (draft.passwordStorage !== 'cloud') return null;
  const { region, awsProfile, tenant, account } = draft.cloudAuth;
  switch (effectiveProvider(draft)) {
    case 'aws':
      return {
        provider: 'aws',
        region: region.trim(),
        profile: optional(awsProfile),
      };
    case 'azure':
      return { provider: 'azure', tenant: optional(tenant) };
    case 'google':
      return { provider: 'google', account: optional(account) };
  }
}

/** Restore only the non-secret settings into an editable draft. */
export function cloudAuthDraft(profile: ProfileDto): CloudAuthDraft {
  const auth = profile.cloudAuth;
  const empty = emptyCloudAuth();
  switch (auth?.provider) {
    case 'aws':
      return {
        ...empty,
        provider: 'aws',
        region: auth.region,
        awsProfile: auth.profile ?? '',
      };
    case 'azure':
      return { ...empty, provider: 'azure', tenant: auth.tenant ?? '' };
    case 'google':
      return { ...empty, provider: 'google', account: auth.account ?? '' };
    case undefined:
      return empty;
  }
}

export function cloudAuthErrors(draft: Draft): DraftErrors {
  if (draft.passwordStorage !== 'cloud') return {};
  if (
    effectiveProvider(draft) === 'aws' &&
    !/^[a-z0-9][a-z0-9-]*$/.test(draft.cloudAuth.region.trim())
  )
    return { cloudRegion: t('useRegionSuch') };
  return {};
}

/** Azure SQL takes its identity from the token, so no user name is needed. */
export function tokenLoginWithoutUser(draft: Draft): boolean {
  return draft.passwordStorage === 'cloud' && draft.engine === 'sqlserver';
}

const VERIFIES: readonly string[] = ['verify-ca', 'verify-full'];
const VERIFIED_TLS = t('cloudTokenOnly');

/** Mirrors `CloudAuth::require_secure_route` (docs/24 §19). */
function routeIsSecure(draft: Draft, provider: CloudProvider): boolean {
  const authProxy =
    draft.helper?.kind === 'cloud' && draft.helper.flavor === 'cloudsql';
  if (provider === 'google' && draft.engine === 'postgres' && authProxy)
    return true;
  const hasCa = draft.tls.caFile.trim() !== '' || draft.tls.cloudCa !== null;
  return (
    VERIFIES.includes(draft.tlsMode) || (draft.tlsMode === 'require' && hasCa)
  );
}

/** What the core will refuse at connect time, while it can still be fixed. */
export function insecureRouteAdvice(draft: Draft): string | null {
  if (draft.passwordStorage !== 'cloud') return null;
  const provider = effectiveProvider(draft);
  if (routeIsSecure(draft, provider)) return null;
  if (provider === 'aws')
    return t('verifiedTlsChoose', { verifiedTls: VERIFIED_TLS });
  if (provider === 'azure')
    return draft.engine === 'sqlserver'
      ? t('verifiedTlsSet', { verifiedTls: VERIFIED_TLS })
      : t('verifiedTlsChooseVerify', { verifiedTls: VERIFIED_TLS });
  return draft.engine === 'postgres'
    ? t('verifiedTlsChooseVerifyCa', { verifiedTls: VERIFIED_TLS })
    : t('verifiedTlsChooseVerifyCaInstance', { verifiedTls: VERIFIED_TLS });
}
