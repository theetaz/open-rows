// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { once } from 'node:events';

export const LARGE_ROWS =
  process.platform === 'linux' || process.env.OPENROWS_E2E_FULL_ROWS === '1'
    ? 5000000
    : 10000;
export const LARGE_SQL = `SELECT n FROM generate_series(1,${LARGE_ROWS}) AS n`;

async function checksum(stream) {
  const hash = createHash('sha256');
  for await (const bytes of stream) hash.update(bytes);
  return hash.digest('hex');
}

async function queryAndScroll(api) {
  await api.evaluate(
    `e2e.waitFor(() => document.querySelector('[aria-label="Switch to E2E SSH PostgreSQL"]')).then(button => button.click())`,
  );
  await api.evaluate(
    `e2e.waitFor(() => document.querySelector('[aria-label="Run current statement"]:not(:disabled)')).then(button => button.click())`,
  );
  const result = await api.evaluate(
    `e2e.waitFor(async () => (await e2e.invoke('automation_observations')).query.find(event => event.type === 'resultEnd'),120000)`,
  );
  assert.equal(result.rows, LARGE_ROWS);
  assert.equal(result.truncated, false);
  await api.evaluate(`e2e.waitFor(() => document.querySelector('[role="grid"][aria-rowcount="${LARGE_ROWS + 1}"]')).then(grid => {
    const scroller = [...grid.parentElement.children].find(element => element.style.overflow === 'auto');
    if (!scroller) throw new Error('Grid scroller missing');
    scroller.scrollTop = scroller.scrollHeight;
  })`);
  assert.equal(
    await api.evaluate(`e2e.waitFor(async () => (await e2e.invoke('automation_observations')).windows.some(request =>
    request.resultId === '${result.resultId}' && request.rowStart + request.rowCount >= ${LARGE_ROWS}))`),
    true,
  );
  const bytes = await api.evaluate(
    `e2e.invoke('result_window',{request:{resultId:'${result.resultId}',rowStart:${LARGE_ROWS - 1},rowCount:1,colStart:0,colCount:1}}).then(bytes => bytes.byteLength ?? bytes.length)`,
  );
  assert.ok(bytes > 0);
  return result;
}

async function exportResult(api, scope, result) {
  const request = {
    source: {
      kind: 'result',
      sessionId: scope.sessionId,
      resultId: result.resultId,
    },
    fileName: 'e2e-export',
    format: {
      kind: 'csv',
      delimiter: 'comma',
      options: {
        quote: 'double',
        header: false,
        nullText: '',
        lineEnding: 'lf',
        encoding: 'utf8',
      },
    },
  };
  const started = await api.evaluate(
    `e2e.start('export_start',${JSON.stringify({ request })})`,
  );
  assert.equal(started.status, 'started');
  const events = await api.finish(started.exportId);
  assert.equal(events.at(-1).type, 'finished', JSON.stringify(events));
  assert.equal(events.at(-1).rows, LARGE_ROWS);
  const exported = await checksum(createReadStream(events.at(-1).path));
  return exported;
}

async function serverChecksum(database) {
  const server = spawn(
    'psql',
    [
      '-X',
      '-q',
      '-h',
      '127.0.0.1',
      '-p',
      String(database.port),
      '-U',
      'postgres',
      '-d',
      'postgres',
      '-c',
      `COPY (${LARGE_SQL}) TO STDOUT WITH (FORMAT CSV)`,
    ],
    { stdio: ['ignore', 'pipe', 'inherit'] },
  );
  const exited = once(server, 'exit');
  const expected = await checksum(server.stdout);
  assert.equal((await exited)[0], 0);
  return expected;
}

/** Verify the UI result and export bytes against the real server. */
export async function streamAndExport(api, scope, database) {
  const result = await queryAndScroll(api);
  const exported = await exportResult(api, scope, result);
  assert.equal(
    exported,
    await serverChecksum(database),
    'Export bytes differ from server COPY',
  );
}
