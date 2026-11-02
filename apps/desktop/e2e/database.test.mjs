// SPDX-License-Identifier: Apache-2.0
import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { once } from 'node:events';
import { sshBanner, command } from './database.mjs';

test('SSH readiness waits for a real banner and rejects a closed proxy socket', async () => {
  const server = createServer((socket) => socket.end());
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  try {
    await assert.rejects(
      sshBanner(server.address().port),
      /closed before its banner/,
    );
  } finally {
    server.close();
    await once(server, 'close');
  }
  const ready = createServer((socket) => socket.end('SSH-2.0-fixture\r\n'));
  ready.listen(0, '127.0.0.1');
  await once(ready, 'listening');
  try {
    await sshBanner(ready.address().port);
  } finally {
    ready.close();
    await once(ready, 'close');
  }
});

test('commands with inherited stdout do not require a captured string', () => {
  assert.equal(command(process.execPath, ['-e', ''], { stdio: 'inherit' }), '');
  assert.throws(
    () => command(process.execPath, ['-e', 'process.exit(2)']),
    /failed \(2\)/,
  );
});
