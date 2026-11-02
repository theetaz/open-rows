// SPDX-License-Identifier: Apache-2.0
import { connect } from 'node:net';
import { once } from 'node:events';
import { watch } from 'node:fs';
import { access } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { pageSource } from './page.mjs';
import { nativeEnvironment } from './environment.mjs';

export async function waitForFile(path, directory, process) {
  const watcher = watch(directory);
  const controller = new globalThis.AbortController();
  const deadline = setTimeout(() => controller.abort(), 15000);
  try {
    for (;;) {
      try {
        await access(path);
        return;
      } catch {
        /* Wait for the creating event. */
      }
      if (process.exitCode !== null || process.signalCode !== null)
        throw new Error(`App exited: ${process.exitCode}`);
      await once(watcher, 'change', { signal: controller.signal });
    }
  } finally {
    clearTimeout(deadline);
    watcher.close();
  }
}

export function socketEvaluate(path, source, metadata = false, exit = false) {
  return new Promise((resolve, reject) => {
    const socket = connect(path);
    let reply = '';
    socket.setTimeout(exit ? 5000 : 150000, () =>
      socket.destroy(new Error('Automation request timed out')),
    );
    socket.on('connect', () =>
      socket.write(JSON.stringify({ source, exit }) + '\n'),
    );
    socket.on('data', (chunk) => {
      reply += chunk;
    });
    socket.on('error', reject);
    socket.on('end', () => {
      try {
        const result = JSON.parse(reply);
        if (result.ok) resolve(metadata ? result : result.value);
        else reject(new Error(result.error));
      } catch (error) {
        reject(error);
      }
    });
  });
}

async function openMac(binary, root) {
  const child = spawn(binary, [], {
    env: nativeEnvironment(root),
    stdio: ['ignore', 'ignore', 'inherit'],
  });
  const path = `${root}/bridge.sock`;
  const alive = () => child.exitCode === null && child.signalCode === null;
  try {
    await waitForFile(path, root, child);
    const evaluate = (source) => socketEvaluate(path, source);
    await evaluate(pageSource());
    return {
      evaluate,
      crash: () => {
        const exited = once(child, 'exit');
        child.kill('SIGKILL');
        return exited;
      },
      close: async () => {
        if (!alive()) return;
        const exited = once(child, 'exit');
        try {
          await socketEvaluate(path, 'null', false, true);
        } catch (error) {
          child.kill();
          await exited;
          throw error;
        }
        await exited;
      },
    };
  } catch (error) {
    if (alive()) {
      const exited = once(child, 'exit');
      child.kill();
      await exited;
    }
    throw error;
  }
}

async function waitForExit(pid) {
  const deadline = Date.now() + 10000;
  for (;;) {
    try {
      process.kill(pid, 0);
    } catch (error) {
      if (error.code === 'ESRCH') return;
      throw error;
    }
    if (Date.now() >= deadline) throw new Error('Killed app did not exit');
    await new Promise(globalThis.setImmediate);
  }
}

async function openLinux(binary, root, port) {
  const { remote } = await import('webdriverio');
  const browser = await remote({
    hostname: '127.0.0.1',
    port,
    logLevel: 'error',
    capabilities: {
      'tauri:options': { application: binary },
      browserName: 'wry',
    },
  });
  try {
    await browser.setTimeout({ script: 150000 });
    const evaluate = async (source) => {
      const result =
        await browser.executeAsync(`const done = arguments[arguments.length - 1];
        Promise.resolve(${source}).then(value => done({ok:true,value}), error => done({ok:false,error:JSON.stringify(error,Object.getOwnPropertyNames(error))}));`);
      if (!result.ok) throw new Error(result.error);
      return result.value;
    };
    await evaluate(pageSource());
    const { pid } = await socketEvaluate(`${root}/bridge.sock`, 'null', true);
    return {
      evaluate,
      browser,
      crash: async () => {
        process.kill(pid, 'SIGKILL');
        await waitForExit(pid);
      },
      close: () => browser.deleteSession(),
    };
  } catch (error) {
    await browser.deleteSession();
    throw error;
  }
}

/** Open the real app using the native transport supported by this host OS. */
export async function openApplication(binary, root, port = 4444) {
  if (process.platform === 'darwin') return openMac(binary, root);
  if (process.platform === 'linux') return openLinux(binary, root, port);
  throw new Error('This E2E lane supports macOS and Linux');
}
