// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';

export function workspace(scope, sql, title = 'E2E recovery') {
  const key = `${scope.profile.id}\npostgres`;
  return JSON.stringify({
    version: 1,
    tabs: [
      {
        id: 'query-1',
        kind: 'query',
        connectionId: scope.profile.id,
        database: 'postgres',
        title,
        sql,
      },
    ],
    active: { [key]: 'query-1' },
    next: 2,
  });
}

export async function journal(api, scope) {
  const original = await api.invoke('table_edit_row', {
    target: scope.target,
    keys: { id: '1' },
  });
  const edits = {
    key: ['id'],
    changes: [
      {
        id: JSON.stringify({ id: '1' }),
        number: 1,
        kind: 'update',
        original,
        values: { name: { kind: 'text', value: 'pending crash edit' } },
      },
    ],
    savedAtMs: 0,
  };
  await api.invoke('table_edits_save', { target: scope.target, edits });
  return edits;
}

export async function lostTransaction(api, scope, database) {
  const edits = await journal(api, scope);
  assert.equal(
    (
      await api.run(
        scope.sessionId,
        "BEGIN; UPDATE e2e_rows SET name='uncommitted' WHERE id=1",
      )
    ).at(-1).outcome,
    'completed',
  );
  const pid = await api.scalar(
    scope.sessionId,
    'SELECT pg_backend_pid()::text',
  );
  assert.equal(
    database.sql(`SELECT pg_terminate_backend(${Number(pid)})`),
    't',
  );
  const attempt = await api.evaluate(`e2e.query('${scope.sessionId}','SELECT 1')
    .then(reply => ({reply}),error => ({error}))`);
  let message;
  if (attempt.error) {
    assert.equal(attempt.error.kind, 'network');
    message = attempt.error.message;
  } else {
    assert.equal(attempt.reply.kind, 'started');
    const failed = await api.finish(attempt.reply.queryId);
    assert.equal(failed.at(-1).outcome, 'failed');
    message = failed.find((event) => event.type === 'failed').error.message;
  }
  assert.match(
    message,
    /transaction.*(lost|rolled|open)|(lost|rolled).*transaction/i,
  );
  assert.equal(database.sql('SELECT name FROM e2e_rows WHERE id=1'), 'edited');
  const saved = await api.invoke('table_edits_saved', { target: scope.target });
  assert.deepEqual(saved.changes, edits.changes);
}

export async function restoreAfterCrash(api, scope, expected, sql) {
  assert.equal(await api.invoke('crash_recovery_status'), true);
  const restored = await api.invoke('table_edits_saved', {
    target: scope.target,
  });
  assert.deepEqual(restored.changes, expected.changes);
  const saved = JSON.parse(
    await api.invoke('workspace_state_read', { part: 'queryTabs' }),
  );
  assert.equal(saved.tabs[0].sql, sql);
  await api.evaluate(
    `e2e.waitFor(() => document.querySelector('[aria-label="Switch to E2E SSH PostgreSQL"]')).then(button => button.click())`,
  );
  const editor = await api.evaluate(
    `e2e.waitFor(() => document.querySelector('.cm-content')?.textContent)`,
  );
  assert.equal(editor, sql, 'Unsaved query was not restored into the editor');
}

/** A real crash first requires acknowledging the app's recovery sheet. */
export async function dismissRecovery(api) {
  await api.evaluate(`e2e.waitFor(() => [...document.querySelectorAll('dialog[open], [role="dialog"]')]
    .find(dialog => dialog.textContent.includes('OpenRows closed unexpectedly'))).then(dialog => {
      const done = [...dialog.querySelectorAll('button')].find(button => button.textContent === 'Done');
      if (!done) throw new Error('Recovery acknowledgement is missing');
      done.click();
    })`);
}
