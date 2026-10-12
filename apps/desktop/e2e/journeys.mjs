// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';

export function client(application) {
  const evaluate = async (source) => {
    try {
      return await application.evaluate(source);
    } catch (error) {
      throw new Error(
        `Webview step failed: ${source.slice(0, 180)}: ${error.message}`,
        {
          cause: error,
        },
      );
    }
  };
  const invoke = (name, args = {}) =>
    evaluate(`e2e.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`);
  const finish = (id) => evaluate(`e2e.finish(${JSON.stringify(id)})`);
  const run = async (sessionId, sql, extra = {}) => {
    const reply = await evaluate(
      `e2e.query(${JSON.stringify(sessionId)},${JSON.stringify(sql)},${JSON.stringify(extra)})`,
    );
    assert.equal(reply.kind, 'started', JSON.stringify(reply));
    return finish(reply.queryId);
  };
  const scalar = async (sessionId, sql) => {
    const events = await run(sessionId, sql);
    assert.equal(events.at(-1).outcome, 'completed', JSON.stringify(events));
    const result = events.find((event) => event.type === 'resultEnd');
    return evaluate(`e2e.cell(${JSON.stringify(result.resultId)})`);
  };
  return { evaluate, invoke, finish, run, scalar };
}

export function profileInput(database, extra = {}) {
  return {
    name: 'E2E PostgreSQL',
    env: { kind: 'dev', label: null, color: null },
    group: null,
    host: '127.0.0.1',
    port: database.port,
    database: 'postgres',
    user: 'postgres',
    tlsMode: 'disable',
    safety: 'unrestricted',
    password: { kind: 'clear' },
    passwordStorage: 'none',
    recordHistory: false,
    ...extra,
  };
}

function sshProfile(database) {
  const input = profileInput(database, {
    name: 'E2E SSH PostgreSQL',
    host: database.remoteHost,
    port: database.remotePort,
    ssh: {
      host: '127.0.0.1',
      port: database.sshPort,
      user: 'tester',
      auth: { method: 'privateKey', keyFile: database.key },
      jumpHosts: [],
      password: { kind: 'clear' },
      passphrase: { kind: 'clear' },
    },
  });
  return input;
}

export async function connectAndBrowse(api, database) {
  const input = sshProfile(database);
  const profile = await api.invoke('profile_create', { input });
  const opened = await api.invoke('connection_open', {
    profileId: profile.id,
    password: null,
  });
  assert.equal(opened.server.database, 'postgres');
  const { sessionId } = await api.invoke('session_open', {
    profileId: profile.id,
    database: 'postgres',
  });
  const target = {
    profileId: profile.id,
    database: 'postgres',
    schema: 'public',
    table: 'e2e_rows',
  };
  await browseSchema(api, target);
  await filteredPage(api, sessionId, target);
  return { profile, input, sessionId, target };
}

async function browseSchema(api, target) {
  const schemas = await api.invoke('catalog_schemas', {
    profileId: target.profileId,
    database: 'postgres',
    fresh: true,
  });
  assert.ok(schemas.data.some((schema) => schema.name === 'public'));
  const objects = await api.invoke('catalog_objects', {
    profileId: target.profileId,
    database: 'postgres',
    request: { schema: 'public', kinds: ['table'], fresh: true, limit: null },
  });
  assert.ok(objects.data.some((object) => object.name === target.table));
  const structure = await api.invoke('table_structure', { target });
  assert.deepEqual(
    structure.columns.map((column) => column.name),
    ['id', 'name', 'quantity', 'note'],
  );
}

async function filteredPage(api, sessionId, target) {
  const query = {
    target,
    columns: [],
    filters: [{ column: 'id', operator: 'equal', value: '2', second: '' }],
    combinator: 'and',
    sort: { column: 'id', descending: false },
  };
  const page = await api.evaluate(
    `e2e.start('table_page',${JSON.stringify({ request: { sessionId, query, offset: 0, limit: 100 } })})`,
  );
  const events = await api.finish(page.queryId);
  assert.equal(events.find((event) => event.type === 'resultEnd').rows, 1);
  assert.equal(
    await api.evaluate(
      `e2e.cell('${events.find((event) => event.type === 'resultEnd').resultId}',0,1)`,
    ),
    'two',
  );
}

export async function scriptAndCancel(api, scope, database) {
  await mixedScript(api, scope);
  await cancelAndReuse(api, scope, database);
}

async function mixedScript(api, scope) {
  const events = await api.run(
    scope.sessionId,
    'SELECT 11::text AS value; SELECT * FROM e2e_missing; SELECT 33::text AS value',
  );
  assert.deepEqual(
    events
      .filter((event) => event.type === 'resultEnd')
      .map((event) => event.statement),
    [0, 2],
  );
  const failed = events.find((event) => event.type === 'failed');
  assert.equal(failed.statement, 1);
  assert.equal(failed.error.code, '42P01');
  for (const [index, expected] of [
    [0, '11'],
    [2, '33'],
  ]) {
    const result = events.find(
      (event) => event.type === 'resultEnd' && event.statement === index,
    );
    assert.equal(
      await api.evaluate(`e2e.cell('${result.resultId}')`),
      expected,
    );
  }
}

async function cancelAndReuse(api, scope, database) {
  const slow = await api.evaluate(
    `e2e.query('${scope.sessionId}', 'SELECT pg_sleep(60) /* openrows_e2e_cancel */')`,
  );
  const deadline = Date.now() + 10000;
  while (
    database.sql(
      "SELECT count(*) FROM pg_stat_activity WHERE state='active' AND wait_event='PgSleep' AND query LIKE '%/* openrows_e2e_cancel */%' AND pid <> pg_backend_pid()",
    ) !== '1'
  ) {
    assert.ok(Date.now() < deadline, 'Long query never became active');
  }
  await api.invoke('query_cancel', { queryId: slow.queryId });
  const cancelled = await api.finish(slow.queryId);
  assert.equal(cancelled.at(-1).outcome, 'cancelled');
  assert.equal(await api.scalar(scope.sessionId, 'SELECT 42::text'), '42');
}
