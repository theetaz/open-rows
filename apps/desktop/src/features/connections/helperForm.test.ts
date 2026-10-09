// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { draftFromProfile, emptyDraft, toInput, validate } from './form';
import { helperError, newHelper } from './helperForm';
import { profile } from '../../test/ipcStub';

it('round trips structured helpers separately from the database URL', () => {
  const helper = {
    ...newHelper('kubectl'),
    kind: 'kubectl' as const,
    context: 'test',
    namespace: 'default',
    resource: 'svc/db',
    remotePort: 5432,
    kubeconfig: null,
  };
  const draft = draftFromProfile(profile('routed', { helper }));
  expect(toInput(draft).helper).toEqual(helper);
  expect(helperError(helper, false)).toBeUndefined();
  expect(helperError(helper, true)).toContain('cannot combine');
});
it('requires structured fields and rejects option-like values', () => {
  for (const kind of ['ssm', 'kubectl', 'cloud', 'cloudflared'] as const)
    expect(helperError(newHelper(kind), false)).toBeDefined();
  const helper = { kind: 'cloudflared' as const, hostname: '--token=x' };
  expect(helperError(helper, false)).toContain('cannot start');
  const draft = {
    ...emptyDraft(),
    name: 'test',
    user: 'app',
    helper: { kind: 'cloudflared' as const, hostname: 'db.example.invalid' },
  };
  expect(
    validate({ ...draft, ssh: { ...draft.ssh, enabled: true, host: 'jump' } })
      .helper,
  ).toBeDefined();
});

it('validates custom templates and preserves literal shell characters', () => {
  const helper = {
    kind: 'custom' as const,
    program: '/tools/hop',
    args: ['a;b $(x) {local_port}', '{remote_host}:{remote_port}'],
    readyTimeoutSeconds: null,
  };
  expect(helperError(helper, false)).toBeUndefined();
  expect(helperError({ ...helper, program: 'relative' }, false)).toContain(
    'absolute',
  );
  expect(helperError({ ...helper, args: ['no port'] }, false)).toContain(
    'where to listen',
  );
  expect(
    helperError({ ...helper, args: ['{unknown}{local_port}'] }, false),
  ).toContain('Use only');
  expect(helperError({ ...helper, readyTimeoutSeconds: 301 }, false)).toContain(
    '1–300',
  );
  expect(
    toInput(draftFromProfile(profile('custom', { helper }))).helper,
  ).toEqual(helper);
});
