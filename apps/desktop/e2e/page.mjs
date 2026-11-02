// SPDX-License-Identifier: Apache-2.0
function createChannel(native) {
  return (receive) => {
    let next = 0;
    const pending = new Map();
    const id = native.transformCallback((raw) => {
      pending.set(raw.index, raw);
      while (pending.has(next)) {
        const item = pending.get(next);
        pending.delete(next++);
        if ('end' in item) native.unregisterCallback(id);
        else receive(item.message);
      }
    });
    return `__CHANNEL__:${id}`;
  };
}

function createStreams(invoke, channel) {
  const runs = new Map();
  const start = async (command, args) => {
    const events = [];
    let finish;
    const complete = new Promise((resolve) => {
      finish = resolve;
    });
    const onEvent = channel((event) => {
      events.push(event);
      const terminal = ['finished', 'failed', 'cancelled'].includes(event.type);
      if (terminal && (command === 'export_start' || event.type === 'finished'))
        finish(events);
    });
    const reply = await invoke(command, { ...args, onEvent });
    const id = reply.queryId ?? reply.exportId;
    if (id) runs.set(id, { events, complete });
    return reply;
  };
  const query = (sessionId, sql, extra = {}) =>
    start('query_execute', {
      request: {
        sessionId,
        sql,
        mode: 'all',
        cursor: null,
        confirmToken: null,
        maxRows: null,
        returnOnError: false,
        ...extra,
      },
    });
  return { runs, start, query, finish: (id) => runs.get(id).complete };
}

async function waitFor(condition, timeout = 10000) {
  let timer;
  let stopped = false;
  const poll = async () => {
    while (!stopped) {
      const value = await condition();
      if (value) return value;
      await new Promise(globalThis.requestAnimationFrame);
    }
  };
  const expired = new Promise((_, reject) => {
    timer = globalThis.setTimeout(
      () => reject(new Error('Application condition did not become true')),
      timeout,
    );
  });
  try {
    return await Promise.race([poll(), expired]);
  } finally {
    stopped = true;
    globalThis.clearTimeout(timer);
  }
}

/** Install helpers in the real application webview, preserving all native calls. */
export function installPage() {
  const native = globalThis.__TAURI_INTERNALS__;
  const invoke = (command, args = {}) => native.invoke(command, args);
  globalThis.e2e = {
    invoke,
    waitFor,
    ...createStreams(invoke, createChannel(native)),
    cell: async (resultId, row = 0, column = 0) => {
      const bytes = await invoke('result_cell_value', {
        request: { resultId, row, column },
      });
      return new globalThis.TextDecoder().decode(
        bytes instanceof ArrayBuffer ? bytes : new Uint8Array(bytes),
      );
    },
  };
  return Boolean(
    globalThis.document.querySelector('[aria-label="Workspaces"]'),
  );
}

/** Serialize the self-contained helper closure for the native webview evaluator. */
export function pageSource() {
  return `(() => {
    const createChannel = ${createChannel.toString()};
    const createStreams = ${createStreams.toString()};
    const waitFor = ${waitFor.toString()};
    return (${installPage.toString()})();
  })()`;
}
