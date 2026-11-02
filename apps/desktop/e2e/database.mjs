// SPDX-License-Identifier: Apache-2.0
import { spawnSync } from 'node:child_process';
import { connect } from 'node:net';
import { readFile } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';

export function command(program, args, options = {}) {
  const result = spawnSync(program, args, { encoding: 'utf8', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${program} failed (${result.status}): ${result.stderr}`);
  return result.stdout?.trim() ?? '';
}

function sqlClient(state) {
  return (statement) =>
    command('psql', [
      '-X',
      '-A',
      '-t',
      '-v',
      'ON_ERROR_STOP=1',
      '-h',
      '127.0.0.1',
      '-p',
      String(state.port),
      '-U',
      'postgres',
      '-d',
      'postgres',
      '-c',
      statement,
    ]);
}

function waitFor(condition) {
  const deadline = Date.now() + 30000;
  for (;;) {
    try {
      return condition();
    } catch (error) {
      if (Date.now() >= deadline) throw error;
    }
  }
}

function postgres(state) {
  if (state.port) return;
  state.network = `openrows-e2e-net-${state.suffix}`;
  command('docker', ['network', 'create', state.network]);
  const name = `openrows-e2e-pg-${state.suffix}`;
  command('docker', [
    'run',
    '-d',
    '--name',
    name,
    '--network',
    state.network,
    '-e',
    'POSTGRES_HOST_AUTH_METHOD=trust',
    '-p',
    '127.0.0.1::5432',
    'postgres:17-alpine',
  ]);
  state.names.push(name);
  state.port = Number(
    command('docker', ['port', name, '5432/tcp']).split(':').at(-1),
  );
  state.remoteHost = name;
  state.remotePort = 5432;
}

export function sshBanner(port) {
  return new Promise((resolve, reject) => {
    const socket = connect(port, '127.0.0.1');
    socket.setTimeout(2000, () => socket.destroy(new Error('No SSH banner')));
    socket.once('error', reject);
    socket.once('close', () =>
      reject(new Error('SSH closed before its banner')),
    );
    socket.once('data', (bytes) => {
      socket.destroy();
      if (bytes.toString().startsWith('SSH-2.0-')) resolve();
      else reject(new Error('Invalid SSH banner'));
    });
  });
}

async function waitForSsh(port) {
  const deadline = Date.now() + 30000;
  for (;;) {
    try {
      await sshBanner(port);
      return;
    } catch (error) {
      if (Date.now() >= deadline) throw error;
    }
  }
}

function startSsh(state, publicKey) {
  state.image = `openrows-e2e-sshd:${state.suffix}`;
  command('docker', [
    'build',
    '-q',
    '-t',
    state.image,
    'crates/openrows-net/tests/fixtures/sshd',
  ]);
  const name = `openrows-e2e-ssh-${state.suffix}`;
  const network = state.network ? ['--network', state.network] : [];
  command('docker', [
    'run',
    '-d',
    '--name',
    name,
    ...network,
    '--add-host',
    'host.docker.internal:host-gateway',
    '-e',
    `TEST_PASSWORD=${randomBytes(24).toString('hex')}`,
    '-e',
    `AUTHORIZED_KEYS=${publicKey}`,
    '-p',
    '127.0.0.1::22',
    state.image,
  ]);
  state.names.push(name);
  return Number(command('docker', ['port', name, '22/tcp']).split(':').at(-1));
}

async function ssh(state, root) {
  const key = `${root}/ssh-key`;
  command('ssh-keygen', ['-q', '-t', 'ed25519', '-N', '', '-f', key]);
  const publicKey = (await readFile(`${key}.pub`, 'utf8')).trim();
  const sshPort = startSsh(state, publicKey);
  await waitForSsh(sshPort);
  const hostKey = command('docker', [
    'exec',
    state.names.at(-1),
    'cat',
    '/etc/ssh/ssh_host_ed25519_key.pub',
  ]);
  return { key, sshPort, hostKey };
}

function stop(state) {
  const failures = [];
  while (state.names.length) {
    try {
      command('docker', [
        'container',
        'remove',
        '--force',
        '--volumes',
        state.names.pop(),
      ]);
    } catch (error) {
      failures.push(error);
    }
  }
  for (const [kind, name] of [
    ['network', state.network],
    ['image', state.image],
  ]) {
    if (!name) continue;
    try {
      command('docker', [kind, 'rm', name]);
    } catch (error) {
      failures.push(error);
    }
  }
  if (failures.length)
    throw new AggregateError(failures, 'E2E fixture cleanup failed');
}

export async function databases(root) {
  const state = {
    names: [],
    suffix: randomBytes(5).toString('hex'),
    port: Number(process.env.OPENROWS_E2E_PG_PORT ?? 0),
    remoteHost: 'host.docker.internal',
    remotePort: Number(process.env.OPENROWS_E2E_PG_PORT ?? 0),
  };
  try {
    postgres(state);
    const sql = sqlClient(state);
    waitFor(() => sql('SELECT 1'));
    const tunnel = await ssh(state, root);
    return {
      ...tunnel,
      sql,
      port: state.port,
      remoteHost: state.remoteHost,
      remotePort: state.remotePort,
      stop: () => stop(state),
    };
  } catch (error) {
    stop(state);
    throw error;
  }
}
