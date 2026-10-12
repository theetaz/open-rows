// SPDX-License-Identifier: Apache-2.0
import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { once } from 'node:events';
import { join } from 'node:path';
import { socketEvaluate, waitForFile } from './transport.mjs';
import { installPage } from './page.mjs';

async function fixture(run) {
  const root = await mkdtemp(join(process.cwd(), '.e2e-unit-'));
  try {
    await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test('socket responses preserve values, metadata and explicit failures', async () => {
  await fixture(async (root) => {
    const server = createServer((socket) => {
      socket.on('data', (data) => {
        const { source } = JSON.parse(data);
        socket.end(
          JSON.stringify(
            source === 'bad'
              ? { ok: false, error: 'denied' }
              : { ok: true, value: ['result'], pid: 123 },
          ),
        );
      });
    });
    server.listen(join(root, 'bridge.sock'));
    await once(server, 'listening');
    try {
      assert.deepEqual(await socketEvaluate(join(root, 'bridge.sock'), 'ok'), [
        'result',
      ]);
      assert.equal(
        (await socketEvaluate(join(root, 'bridge.sock'), 'ok', true)).pid,
        123,
      );
      await assert.rejects(
        socketEvaluate(join(root, 'bridge.sock'), 'bad'),
        /denied/,
      );
    } finally {
      server.close();
      await once(server, 'close');
    }
  });
});

test('readiness follows a file event and detects an exited app', async () => {
  await fixture(async (root) => {
    const path = join(root, 'ready');
    const pending = waitForFile(path, root, {
      exitCode: null,
      signalCode: null,
    });
    await writeFile(path, 'ready');
    await pending;
    await assert.rejects(
      waitForFile(join(root, 'missing'), root, {
        exitCode: 1,
        signalCode: null,
      }),
      /App exited/,
    );
  });
});

test('webview channels reorder native messages and finish only on terminal events', async () => {
  const previous = {
    native: globalThis.__TAURI_INTERNALS__,
    document: globalThis.document,
    e2e: globalThis.e2e,
  };
  let callback;
  const cleaned = [];
  globalThis.__TAURI_INTERNALS__ = {
    transformCallback: (receive) => {
      callback = receive;
      return 7;
    },
    unregisterCallback: (id) => cleaned.push(id),
    invoke: async () => ({ kind: 'started', queryId: 'query' }),
  };
  globalThis.document = { querySelector: () => ({}) };
  try {
    assert.equal(installPage(), true);
    await globalThis.e2e.query('session', 'SELECT 1');
    callback({ index: 1, message: { type: 'finished', outcome: 'completed' } });
    callback({ index: 0, message: { type: 'resultEnd', rows: 1 } });
    const events = await globalThis.e2e.finish('query');
    assert.deepEqual(
      events.map((event) => event.type),
      ['resultEnd', 'finished'],
    );
    callback({ index: 2, end: true });
    assert.deepEqual(cleaned, [7]);
  } finally {
    globalThis.__TAURI_INTERNALS__ = previous.native;
    globalThis.document = previous.document;
    globalThis.e2e = previous.e2e;
  }
});
