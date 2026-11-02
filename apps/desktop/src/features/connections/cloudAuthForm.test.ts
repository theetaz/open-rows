// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import {
  cloudAuthDraft,
  cloudAuthFields,
  cloudProviders,
  insecureRouteAdvice,
} from './cloudAuthForm';
import { emptyDraft, toInput, validate } from './form';
import type { Draft } from './form';
import { profile as makeProfile } from '../../test/ipcStub';

function cloud(
  change: Partial<Draft> = {},
  engine: Draft['engine'] = 'postgres',
) {
  const draft = emptyDraft(engine);
  return {
    ...draft,
    name: 'c',
    host: 'db.abc.us-east-1.rds.amazonaws.com',
    user: 'iam_user',
    passwordStorage: 'cloud' as const,
    tlsMode: 'verify-full' as const,
    cloudAuth: { ...draft.cloudAuth, region: 'us-east-1' },
    ...change,
  };
}

describe('cloud token settings', () => {
  it('offers each provider only for the engines it serves', () => {
    const values = (engine: Draft['engine']) =>
      cloudProviders(engine).map((choice) => choice.value);
    expect(values('postgres')).toEqual(['aws', 'azure', 'google']);
    expect(values('mysql')).toEqual(['aws', 'azure', 'google']);
    expect(values('sqlserver')).toEqual(['azure']);
    for (const engine of [
      'mariadb',
      'sqlite',
      'clickhouse',
      'mongodb',
      'libsql',
      'libsql-file',
      'duckdb',
    ] as const)
      expect(values(engine)).toEqual([]);
  });

  it('sends only the chosen provider and never a password', () => {
    const input = toInput(
      cloud({
        cloudAuth: {
          provider: 'aws',
          region: ' us-east-1 ',
          awsProfile: ' dev ',
          tenant: 'ignored',
          account: 'ignored',
        },
        password: 'typed',
        passwordMode: 'set',
      }),
    );
    expect(input.cloudAuth).toEqual({
      provider: 'aws',
      region: 'us-east-1',
      profile: 'dev',
    });
    expect(input.password).toEqual({ kind: 'clear' });
  });

  it('clears the settings when another password policy is chosen', () => {
    expect(cloudAuthFields(cloud({ passwordStorage: 'keychain' }))).toBeNull();
  });

  it('uses Entra ID on SQL Server and needs no user there', () => {
    const draft = cloud({ user: '' }, 'sqlserver');
    expect(cloudAuthFields(draft)).toEqual({ provider: 'azure', tenant: null });
    expect(validate(draft).user).toBeUndefined();
    expect(validate(cloud({ user: '' })).user).toBe('User is required');
  });

  it('requires a region for AWS only', () => {
    const empty = cloud();
    empty.cloudAuth = { ...empty.cloudAuth, region: '' };
    expect(validate(empty).cloudRegion).toBeDefined();
    empty.cloudAuth = { ...empty.cloudAuth, provider: 'google' };
    expect(validate(empty).cloudRegion).toBeUndefined();
  });

  it('restores saved settings', () => {
    const profile = makeProfile('x');
    expect(
      cloudAuthDraft({
        ...profile,
        cloudAuth: { provider: 'azure', tenant: 'contoso.example' },
      }),
    ).toMatchObject({ provider: 'azure', tenant: 'contoso.example' });
    expect(
      cloudAuthDraft({
        ...profile,
        cloudAuth: { provider: 'aws', region: 'eu-west-1', profile: null },
      }),
    ).toMatchObject({ provider: 'aws', region: 'eu-west-1', awsProfile: '' });
    expect(cloudAuthDraft(profile).provider).toBe('aws');
  });
});

describe('cloud token route advice', () => {
  it('warns exactly when the core would refuse the route', () => {
    for (const tlsMode of ['disable', 'allow', 'prefer', 'require'] as const)
      expect(insecureRouteAdvice(cloud({ tlsMode }))).toContain('verified TLS');
    for (const tlsMode of ['verify-ca', 'verify-full'] as const)
      expect(insecureRouteAdvice(cloud({ tlsMode }))).toBeNull();
    const draft = cloud({ tlsMode: 'require' });
    expect(
      insecureRouteAdvice({
        ...draft,
        tls: { ...draft.tls, cloudCa: 'awsRds' },
      }),
    ).toBeNull();
    expect(insecureRouteAdvice(cloud({ passwordStorage: 'none' }))).toBeNull();
  });

  it('accepts only the Google token through the Cloud SQL Auth Proxy', () => {
    const helper = {
      kind: 'cloud',
      instance: 'p:r:i',
      flavor: 'cloudsql',
      iamAuth: false,
      privateIp: true,
      credentialsFile: null,
    } as const;
    const base = cloud({ tlsMode: 'disable', helper });
    expect(insecureRouteAdvice(base)).toContain('AWS RDS');
    expect(
      insecureRouteAdvice({
        ...base,
        cloudAuth: { ...base.cloudAuth, provider: 'google' },
      }),
    ).toBeNull();
    expect(
      insecureRouteAdvice({
        ...base,
        cloudAuth: { ...base.cloudAuth, provider: 'google' },
        helper: { ...helper, flavor: 'alloydb' },
      }),
    ).toContain('Cloud SQL Auth Proxy');
    expect(
      insecureRouteAdvice({
        ...base,
        engine: 'mysql',
        cloudAuth: { ...base.cloudAuth, provider: 'google' },
      }),
    ).toContain('Automatic IAM');
  });
});
