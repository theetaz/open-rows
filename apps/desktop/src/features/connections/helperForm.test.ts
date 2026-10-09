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
