// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';
import { profileInput } from './journeys.mjs';

const text = (value) => ({ kind: 'text', value: String(value) });

async function commitRequest(api, scope) {
  const original = await api.invoke('table_edit_row', {
    target: scope.target,
    keys: { id: '1' },
  });
  const deleted = await api.invoke('table_edit_row', {
    target: scope.target,
    keys: { id: '3' },
  });
  const changes = [
    {
      kind: 'update',
      original,
      values: {
        name: text('edited'),
        quantity: text(9),
        note: text('changed'),
      },
    },
    {
      kind: 'insert',
      original: {},
      values: {
        id: text(4),
        name: text('four'),
        quantity: text(4),
        note: text('new'),
      },
    },
    { kind: 'delete', original: deleted, values: {} },
  ];
  return {
    sessionId: scope.sessionId,
    target: scope.target,
    changes,
    confirmToken: null,
  };
}

async function assertRollback(api, request, database, staleToken) {
  const current = await api.invoke('table_edit_row', {
    target: request.target,
    keys: { id: '1' },
  });
  const bad = {
    ...request,
    changes: [
      {
        kind: 'update',
        original: current,
        values: { name: text('must rollback') },
      },
      {
        kind: 'insert',
        original: {},
        values: { id: text(2), name: text('duplicate'), quantity: text(2) },
      },
    ],
  };
  const prepared = await api.invoke('table_edit', { request: bad });
  await assert.rejects(
    api.invoke('table_edit', {
      request: { ...bad, confirmToken: prepared.token },
    }),
    /23505|duplicate key/,
  );
  assert.equal(
    database.sql('SELECT name FROM e2e_rows WHERE id=1'),
    'edited',
    'Failed batch did not roll back',
  );
  await assertStaleToken(api, request, { bad, staleToken }, database);
}

async function assertStaleToken(api, request, { bad, staleToken }, database) {
  const invalid = await api.invoke('table_edit', {
    request: { ...request, confirmToken: staleToken + 'invalid' },
  });
  assert.equal(invalid.kind, 'prepared');
  const stale = await api.invoke('table_edit', {
    request: { ...bad, confirmToken: staleToken },
  });
  assert.equal(stale.kind, 'prepared');
  assert.equal(database.sql('SELECT name FROM e2e_rows WHERE id=1'), 'edited');
}

export async function commitAndRollback(api, scope, database) {
  const request = await commitRequest(api, scope);
  const preview = await api.invoke('table_edit', { request });
  assert.equal(preview.kind, 'prepared');
  assert.equal(preview.statements.length, 3);
  assert.equal(
    database.sql('SELECT name FROM e2e_rows WHERE id=1'),
    'one',
    'Preview wrote data',
  );
  const committed = await api.invoke('table_edit', {
    request: { ...request, confirmToken: preview.token },
  });
  assert.equal(committed.kind, 'committed');
  assert.equal(
    database.sql(
      "SELECT string_agg(id || ':' || name || ':' || quantity || ':' || note, ',' ORDER BY id) FROM e2e_rows",
    ),
    '1:edited:9:changed,2:two:2:old,4:four:4:new',
  );
  await assertRollback(api, request, database, preview.token);
}

export async function safeMode(api, database) {
  const input = profileInput(database, {
    name: 'E2E Production',
    env: { kind: 'prod', label: null, color: null },
    safety: 'confirmWrites',
  });
  const profile = await api.invoke('profile_create', { input });
  const { sessionId } = await api.invoke('session_open', {
    profileId: profile.id,
    database: 'postgres',
  });
  const sql = 'DELETE FROM e2e_rows';
  const confirm = await api.evaluate(
    `e2e.query('${sessionId}',${JSON.stringify(sql)})`,
  );
  assert.equal(confirm.kind, 'confirmationRequired');
  assert.equal(database.sql('SELECT count(*) FROM e2e_rows'), '3');
  const changed = await api.evaluate(
    `e2e.query('${sessionId}','DROP TABLE e2e_rows',{confirmToken:${JSON.stringify(confirm.token)}})`,
  );
  assert.equal(changed.kind, 'confirmationRequired');
  const readOnly = await api.invoke('profile_update', {
    id: profile.id,
    input: { ...input, safety: 'readOnly' },
  });
  assert.equal(readOnly.safety, 'readOnly');
  const blocked = await api.evaluate(
    `e2e.query('${sessionId}',${JSON.stringify(sql)},{confirmToken:${JSON.stringify(confirm.token)}})`,
  );
  assert.equal(blocked.kind, 'blocked');
  await assertReadOnly(api, profile, sessionId, database);
}

async function assertReadOnly(api, profile, sessionId, database) {
  const target = {
    profileId: profile.id,
    database: 'postgres',
    schema: 'public',
    table: 'e2e_rows',
  };
  await assert.rejects(
    api.invoke('table_edit', {
      request: {
        sessionId,
        target,
        changes: [{ kind: 'delete', original: { id: '1' }, values: {} }],
        confirmToken: null,
      },
    }),
    /read-only/,
  );
  assert.equal(
    await api.scalar(sessionId, 'SELECT count(*)::text FROM e2e_rows'),
    '3',
  );
  assert.equal(database.sql('SELECT count(*) FROM e2e_rows'), '3');
}
