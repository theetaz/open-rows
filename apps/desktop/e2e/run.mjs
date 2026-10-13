// SPDX-License-Identifier: Apache-2.0
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { once } from 'node:events';
import { command, databases } from './database.mjs';
import { assertAutomationBinary } from './artifact.mjs';
import { nativeEnvironment } from './environment.mjs';
import { openApplication } from './transport.mjs';
import { client, connectAndBrowse, scriptAndCancel } from './journeys.mjs';
import { commitAndRollback, safeMode } from './safety.mjs';
import {
  journal,
  lostTransaction,
  workspace,
  restoreAfterCrash,
  dismissRecovery,
} from './recovery.mjs';
import { LARGE_ROWS, LARGE_SQL, streamAndExport } from './export.mjs';

async function build() {
  if (process.env.OPENROWS_E2E_BINARY)
    return resolve(process.env.OPENROWS_E2E_BINARY);
  const triple =
    process.env.CARGO_BUILD_TARGET ??
    command('rustc', ['-vV']).match(/^host: (.+)$/m)?.[1];
  if (!triple) throw new Error('Cannot resolve the native Rust target');
  const options = {
    stdio: 'inherit',
    env: { ...process.env, CARGO_BUILD_TARGET: triple },
  };
  command('pnpm', ['build'], { stdio: 'inherit' });
  command('bash', ['scripts/stage-host.sh', 'dev'], options);
  command(
    'cargo',
    [
      'build',
      '-p',
      'openrows-app',
      '--features',
      'test-automation,tauri/custom-protocol',
      '--locked',
    ],
    options,
  );
  return resolve(
    process.env.CARGO_TARGET_DIR ?? 'target',
    ...(triple ? [triple] : []),
    'debug/openrows-app',
  );
}

async function unusedPort() {
  const server = createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const port = server.address().port;
  server.close();
  await once(server, 'close');
  return port;
}

async function startDriver(state) {
  if (process.platform !== 'linux') return;
  state.driverPort = await unusedPort();
  const nativePort = await unusedPort();
  state.driver = spawn(
    'tauri-driver',
    ['--port', String(state.driverPort), '--native-port', String(nativePort)],
    {
      env: nativeEnvironment(state.root),
      detached: true,
      stdio: 'inherit',
    },
  );
  const deadline = Date.now() + 15000;
  for (;;) {
    try {
      const response = await fetch(
        `http://127.0.0.1:${state.driverPort}/status`,
      );
      if (response.ok && (await response.json()).value?.ready) return;
    } catch {
      /* Observe driver readiness without delaying the test. */
    }
    if (Date.now() >= deadline || state.driver.exitCode !== null)
      throw new Error('tauri-driver did not become ready');
  }
}

async function prepare(state) {
  await writeFile(join(state.root, '.automation-owned'), '');
  await mkdir(join(state.root, 'data'));
  state.database = await databases(state.root);
  await writeFile(
    join(state.root, 'data/known_hosts'),
    `[127.0.0.1]:${state.database.sshPort} ${state.database.hostKey}\n`,
    { mode: 0o600 },
  );
  await startDriver(state);
  state.database.sql(
    "CREATE TABLE e2e_rows(id integer PRIMARY KEY,name text NOT NULL,quantity integer,note text); INSERT INTO e2e_rows VALUES(1,'one',1,'old'),(2,'two',2,'old'),(3,'three',3,'old')",
  );
  state.ownsTable = true;
  state.app = await openApplication(state.binary, state.root, state.driverPort);
  state.api = client(state.app);
}

async function restart(state, crashed = false) {
  if (crashed) await state.app.crash();
  if (!crashed || state.app.browser) await state.app.close();
  state.app = undefined;
  await rm(join(state.root, 'bridge.sock'), { force: true });
  state.app = await openApplication(state.binary, state.root, state.driverPort);
  state.api = client(state.app);
  if (crashed) await dismissRecovery(state.api);
}

async function firstJourneys(state) {
  state.scope = await connectAndBrowse(state.api, state.database);
  await commitAndRollback(state.api, state.scope, state.database);
  process.stdout.write(
    'PASS journey 1: SSH, schema, filtered page, edit/insert/delete/commit/rollback\n',
  );
  await scriptAndCancel(state.api, state.scope, state.database);
  process.stdout.write(
    'PASS journeys 2–3: mixed script results/errors, cancellation, session reuse\n',
  );
}

async function largeJourney(state) {
  await state.api.invoke('workspace_state_write', {
    part: 'queryTabs',
    state: workspace(state.scope, LARGE_SQL, 'E2E large result'),
  });
  // Preserve the explicit fixture descriptor without a pagehide flush of old UI state.
  await restart(state, true);
  state.scope.sessionId = (
    await state.api.invoke('session_open', {
      profileId: state.scope.profile.id,
      database: 'postgres',
    })
  ).sessionId;
  await streamAndExport(state.api, state.scope, state.database);
  process.stdout.write(
    `PASS journey 4: ${LARGE_ROWS} rows, scroll to last row, CSV server checksum\n`,
  );
}

async function recoveryJourneys(state) {
  await lostTransaction(state.api, state.scope, state.database);
  process.stdout.write(
    'PASS journey 5: broken transaction, rollback, pending edits preserved\n',
  );
  const expected = await journal(state.api, state.scope);
  const draft = 'SELECT 123 AS unsaved_crash_draft';
  await state.api
    .evaluate(`e2e.waitFor(() => document.querySelector('.cm-content')).then(editor => {
    editor.focus(); document.execCommand('selectAll');
    document.execCommand('insertText',false,${JSON.stringify(draft)});
  })`);
  await state.api.evaluate(`e2e.waitFor(async () => {
    const text = await e2e.invoke('workspace_state_read',{part:'queryTabs'});
    return text && JSON.parse(text).tabs[0]?.sql === ${JSON.stringify(draft)};
  })`);
  await restart(state, true);
  await restoreAfterCrash(state.api, state.scope, expected, draft);
  process.stdout.write(
    'PASS journey 6: forced crash, restored editor and pending changes\n',
  );
}

async function stopDriver(state) {
  if (!state.driver?.pid) return;
  const alive =
    state.driver.exitCode === null && state.driver.signalCode === null;
  const stopped = alive ? once(state.driver, 'exit') : Promise.resolve();
  try {
    process.kill(-state.driver.pid, 'SIGTERM');
  } catch (error) {
    if (error.code !== 'ESRCH') throw error;
  }
  await stopped;
}

async function cleanup(state) {
  const failures = [];
  const actions = [
    () => state.app?.close(),
    () => stopDriver(state),
    () => {
      if (state.ownsTable && process.env.OPENROWS_E2E_PG_PORT)
        state.database.sql('DROP TABLE e2e_rows');
    },
    () => state.database?.stop(),
    () => rm(state.root, { recursive: true, force: true }),
  ];
  for (const action of actions) {
    try {
      await action();
    } catch (error) {
      failures.push(error);
    }
  }
  if (failures.length) throw new AggregateError(failures, 'E2E cleanup failed');
}

async function main() {
  const binary = await build();
  await assertAutomationBinary(binary);
  const state = { binary, root: await mkdtemp(join(process.cwd(), '.e2e-')) };
  const started = Date.now();
  try {
    await prepare(state);
    await firstJourneys(state);
    await largeJourney(state);
    await recoveryJourneys(state);
    await safeMode(state.api, state.database);
    process.stdout.write(
      'PASS journey 8: production confirmation, token scope, read-only query/edit denial\n',
    );
    const seconds = (Date.now() - started) / 1000;
    if (seconds >= 300)
      throw new Error(`E2E runtime ${seconds}s exceeds 5 minutes`);
    process.stdout.write(
      `PASS E2E (${seconds.toFixed(1)}s, excluding app build)\n`,
    );
  } catch (error) {
    if (state.app) {
      try {
        process.stderr.write(
          `App state: ${await state.app.evaluate('document.body.innerText.slice(0,2000)')}\n`,
        );
      } catch (diagnostic) {
        process.stderr.write(
          `App diagnostic unavailable: ${diagnostic.message}\n`,
        );
      }
    }
    throw error;
  } finally {
    await cleanup(state);
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error}\n`);
  process.exitCode = 1;
});
