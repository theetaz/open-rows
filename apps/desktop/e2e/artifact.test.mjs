// SPDX-License-Identifier: Apache-2.0
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { assertAutomationBinary } from './artifact.mjs';

test('artifact preflight refuses normal binaries and accepts all bridge markers', async () => {
  const root = await mkdtemp(join(process.cwd(), '.e2e-artifact-'));
  const path = join(root, 'app');
  try {
    await writeFile(path, 'normal executable');
    await assert.rejects(assertAutomationBinary(path), /debug executable/);
    await writeFile(
      path,
      'automation_reply automation_observations OPENROWS_E2E_ROOT bridge.sock',
    );
    await assertAutomationBinary(path);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
