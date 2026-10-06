// SPDX-License-Identifier: Apache-2.0
import { resultWindow } from '@openrows/ipc';
import { TileSource, gridTheme } from '@openrows/grid';
import type { GridProps, MountGrid } from '@openrows/grid';
import { ArrowWorker } from '../results/workerClient';
import { SpikeSession, withTimeout } from './lifecycle';
import { SCENARIOS, ROWS, scenarioDuration } from './gridScenarios';
import { sampleGrid } from './gridSampler';

/** Release-only canvas benchmark; all modules/worker startup precede sampled frames. */
export async function runGridSpike(session: SpikeSession): Promise<void> {
  const worker = new ArrowWorker(),
    host = createHost();
  const stop = (): void => {
    worker.stop();
  };
  session.signal.addEventListener('abort', stop, { once: true });
  let error: string | null = null;
  try {
    session.progress('grid-timer', JSON.stringify(session.timer));
    await withTimeout(
      worker.initialize(),
      10_000,
      'Grid worker initialization',
    );
    const { mountGrid: mount } = await withTimeout(
      import('@openrows/grid/canvas'),
      10_000,
      'Canvas renderer loading',
    );
    await withTimeout(
      runOption({ host, session, worker, option: 'own', mount }),
      60_000,
      'Canvas grid scenarios',
    );
  } catch (caught) {
    error = String(caught);
  } finally {
    session.signal.removeEventListener('abort', stop);
    stop();
    host.remove();
  }
  await session.finish(error);
}
async function runOption(context: Omit<Scenario, 'scenario'>): Promise<void> {
  for (const scenario of SCENARIOS) {
    context.session.signal.throwIfAborted();
    context.session.progress(
      'grid-scenario',
      `${context.option}/${scenario} started`,
    );
    await runScenario({ ...context, scenario });
    context.session.progress(
      'grid-scenario',
      `${context.option}/${scenario} complete`,
    );
  }
}
function createHost(): HTMLElement {
  const host = document.createElement('div');
  host.style.cssText =
    'position:fixed;inset:0;width:1040px;height:600px;z-index:999;';
  document.body.append(host);
  return host;
}
interface Scenario {
  host: HTMLElement;
  session: SpikeSession;
  worker: ArrowWorker;
  option: string;
  mount: MountGrid;
  scenario: string;
}
async function runScenario(context: Scenario): Promise<void> {
  const { host, session, option, mount, scenario } = context;
  const props = scenarioProps(context);
  const source = props.source;
  await withTimeout(
    source.window({ r0: 0, r1: 23, c0: 0, c1: 19 }, 1),
    10_000,
    'Grid initial tiles',
  );

  const handle = mount(host, props);
  try {
    await withTimeout(nextFrames(3), 2_000, 'Grid first paint');
    session.grid.push(
      await withTimeout(
        sampleGrid({
          handle,
          source,
          option,
          scenario,
          signal: session.signal,
        }),
        scenarioDuration(scenario) + 1_000,
        `Grid ${option}/${scenario}`,
      ),
    );
  } finally {
    handle.destroy();
  }
}
function nextFrames(count: number): Promise<void> {
  return new Promise((resolve) => {
    const next = (): void => {
      if (--count === 0) resolve();
      else requestAnimationFrame(next);
    };
    requestAnimationFrame(next);
  });
}

function scenarioSource({ scenario, worker }: Scenario): TileSource {
  const columns = Array.from(
    { length: scenario === 'horizontal' ? 100 : 20 },
    (_, col) => ({
      id: String(col),
      title: `column_${String(col)}`,
      width: 120,
      pinned: col < 2,
    }),
  );
  const source = new TileSource('spike', ROWS, columns, async (request) => {
    // Virtual columns repeat the same twenty mixed S1 columns; requests still traverse IPC + worker.
    const bytes = await withTimeout(
      resultWindow({
        ...request,
        colStart: request.colStart % 20,
      }),
      2_000,
      'Grid tile invoke',
    );
    return await withTimeout(
      worker.display(bytes, request),
      2_000,
      'Grid tile display',
    );
  });
  return source;
}
function scenarioProps(context: Scenario): GridProps {
  const session = context.session;
  const props: GridProps = {
    source: scenarioSource(context),
    theme: gridTheme(
      document.documentElement.dataset.theme === 'dark' ||
        (document.documentElement.dataset.theme !== 'light' &&
          window.matchMedia('(prefers-color-scheme: dark)').matches)
        ? 'dark'
        : 'light',
    ),
    selection: {
      row: 2,
      column: 2,
      ranges: [
        { r0: 1, r1: 3, c0: 1, c1: 3 },
        { r0: 5, r1: 6, c0: 5, c1: 6 },
      ],
    },
    onSelection: (selection) => {
      props.selection = selection;
    },
    onError: (error) => {
      void session.finish(String(error));
    },
  };
  return props;
}
