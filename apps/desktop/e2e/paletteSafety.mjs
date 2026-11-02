// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';
import { join } from 'node:path';

const INPUT = '[role="combobox"]';
const NAME = 'E2E Palette Read-only';
const TABLE = 'e2e_palette';

/** Run a palette command as a user does: open it, type its name, Return. */
function palette(api, command) {
  return api.evaluate(`(async () => {
    document.querySelector('[aria-label="Command palette"]').click();
    const input = await e2e.waitFor(() =>
      document.querySelector('dialog ${INPUT}'));
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    set.call(input, ${JSON.stringify(command)});
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await e2e.waitFor(() => [...document.querySelectorAll('[role="option"]')]
      .some((option) => option.textContent.includes(${JSON.stringify(command)})));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await e2e.waitFor(() => !document.querySelector('dialog ${INPUT}'));
  })()`);
}

function typeSql(api, sql) {
  return api.evaluate(`e2e.waitFor(() => document.querySelector('.cm-content')).then((editor) => {
    editor.focus();
    document.execCommand('selectAll');
    document.execCommand('insertText', false, ${JSON.stringify(sql)});
  })`);
}

/** A SQLite file with three rows, saved as read-only and shown in the rail. */
async function readOnlyFile(api, root) {
  const input = {
    name: NAME,
    group: null,
    env: { kind: 'dev', label: null, color: null },
    engine: 'sqlite',
    file: { path: join(root, `${TABLE}.db`), readOnly: false, create: true },
    host: '',
    port: 0,
    database: null,
    user: null,
    tlsMode: 'disable',
    safety: 'unrestricted',
    password: { kind: 'clear' },
    passwordStorage: 'none',
    recordHistory: false,
  };
  const profile = await api.invoke('profile_create', { input });
  await api.invoke('connection_open', {
    profileId: profile.id,
    password: null,
  });
  const { sessionId } = await api.invoke('session_open', {
    profileId: profile.id,
    database: null,
  });
  await api.run(
    sessionId,
    `CREATE TABLE ${TABLE}(id integer PRIMARY KEY, name text); INSERT INTO ${TABLE} VALUES (1,'a'),(2,'b'),(3,'c')`,
  );
  await api.invoke('session_close', { sessionId });
  await api.invoke('connection_close', { profileId: profile.id });
  await api.invoke('profile_update', {
    id: profile.id,
    input: { ...input, safety: 'readOnly' },
  });
  return profile;
}

async function assertRows(api, profile) {
  const { sessionId } = await api.invoke('session_open', {
    profileId: profile.id,
    database: null,
  });
  assert.equal(
    await api.scalar(sessionId, `SELECT CAST(count(*) AS TEXT) FROM ${TABLE}`),
    '3',
  );
  await api.invoke('session_close', { sessionId });
}

/** Journey 8, palette path: a write from the palette hits the real guard. */
export async function paletteWriteBlocked(api, root) {
  const profile = await readOnlyFile(api, root);
  await api.evaluate(`(async () => {
    window.dispatchEvent(new Event('openrows-profiles-changed'));
    const button = await e2e.waitFor(() =>
      document.querySelector('[aria-label="Switch to ${NAME}"]'));
    button.click();
    await e2e.waitFor(() =>
      document.querySelector('[aria-label="Safe mode: Read-only"]'));
    const picker = await e2e.waitFor(() =>
      document.querySelector('dialog[aria-label="Open database"]'));
    const main = await e2e.waitFor(() => [...picker.querySelectorAll('[role="option"]')]
      .find((option) => option.textContent.includes('main')));
    main.click();
    const open = await e2e.waitFor(() => [...picker.querySelectorAll('button')]
      .find((button) => button.textContent === 'Open' && !button.disabled));
    open.click();
    await e2e.waitFor(() => document.querySelector('[aria-label="Connection context"]')
      ?.textContent.includes(': main'));
  })()`);
  await palette(api, 'New query tab');
  await typeSql(api, `DELETE FROM ${TABLE}`);
  await palette(api, 'Run current statement');
  await api.evaluate(`e2e.waitFor(() =>
    document.body.innerText.includes('Blocked: read-only connection'))`);
  await assertRows(api, profile);
  await api.evaluate(`e2e.waitFor(() => [...document.querySelectorAll('[role="treeitem"]')]
    .find((row) => row.textContent.includes('${TABLE}'))).then((row) => row.click())`);
  await api.evaluate(
    `e2e.waitFor(() => document.querySelector('canvas')).then(() => true)`,
  );
  await palette(api, 'Insert row');
  await palette(api, 'Commit changes');
  const staged = await api.evaluate(
    `document.body.innerText.includes('Review & commit')`,
  );
  assert.equal(staged, false, 'a read-only table stages nothing');
  await assertRows(api, profile);
}
