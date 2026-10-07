// SPDX-License-Identifier: Apache-2.0
import { CONFIGS } from './editorScript';
import { runConfig } from './editorRun';
import type { VimModule } from './editorRun';
import { SpikeSession, withTimeout } from './lifecycle';
import { generateSqlDocument } from './sqlDocument';

/** Per-configuration watchdog: 240 keystrokes at ≈ 70 ms plus mount and parse. */
const CONFIG_DEADLINE_MS = 60_000;

/** Release-only editor benchmark (M0-S5); this module and CodeMirror load lazily. */
export async function runEditorSpike(session: SpikeSession): Promise<void> {
  const host = createHost();
  let error: string | null = null;
  try {
    session.progress('editor-timer', JSON.stringify(session.timer));
    const doc = generateSqlDocument();
    const vim: VimModule = await withTimeout(
      import('@openrows/editor/vim'),
      10_000,
      'Vim module loading',
    );
    for (const config of CONFIGS) {
      session.signal.throwIfAborted();
      session.progress('editor-config', `${config} started`);
      session.editor.push(
        await withTimeout(
          runConfig({
            host,
            session,
            config,
            doc,
            vim: config === 'vim' ? vim : null,
          }),
          CONFIG_DEADLINE_MS,
          `Editor ${config}`,
        ),
      );
      session.progress('editor-config', `${config} complete`);
    }
  } catch (caught) {
    error = String(caught);
  } finally {
    host.remove();
  }
  await session.finish(error);
}

function createHost(): HTMLElement {
  const host = document.createElement('div');
  host.style.cssText =
    'position:fixed;inset:0;width:1040px;height:600px;z-index:999;';
  document.body.append(host);
  return host;
}
