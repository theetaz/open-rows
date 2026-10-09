// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { dumpContext } from '@openrows/ipc';
import type {
  DumpContextDto,
  DumpToolDto,
  DumpToolKindDto,
} from '@openrows/ipc';
import type { DumpTarget } from './dumpStore';
import { messageOf, scopeOf } from './model';
import styles from './Dump.module.css';

type ContextState =
  | { status: 'loading' }
  | { status: 'ready'; context: DumpContextDto }
  | { status: 'failed'; message: string };

/** Engine, safety switches and tools of the target's database. */
export function useDumpContext(target: DumpTarget): ContextState {
  const [state, setState] = useState<ContextState>({ status: 'loading' });
  useEffect(() => {
    let live = true;
    dumpContext(scopeOf(target)).then(
      (context) => {
        if (live) setState({ status: 'ready', context });
      },
      (error: unknown) => {
        if (live) setState({ status: 'failed', message: messageOf(error) });
      },
    );
    return () => {
      live = false;
    };
  }, [target]);
  return state;
}

function ToolItem({ tool }: { tool: DumpToolDto }): React.JSX.Element {
  const where = tool.fromSettings ? ' · set in Settings ▸ Tools' : '';
  return (
    <li>
      <span>{tool.name}</span>
      {tool.error ? (
        <span className={styles.error} role="alert">
          {tool.error}
        </span>
      ) : (
        <span title={tool.path ?? undefined}>
          {tool.version ?? 'Unknown version'}
          {where}
        </span>
      )}
      {tool.warning && (
        <span className={styles.warning} role="status">
          {tool.warning}
        </span>
      )}
    </li>
  );
}

/**
 * The copies of `kinds` a job will run, their versions and why they may
 * not work with the server.
 */
export function ToolList(props: {
  tools: readonly DumpToolDto[];
  kinds: readonly DumpToolKindDto[];
}): React.JSX.Element | null {
  const shown = props.tools.filter((tool) => props.kinds.includes(tool.tool));
  if (!shown.length) return null;
  return (
    <ul className={styles.tools} aria-label="Client tools">
      {shown.map((tool) => (
        <ToolItem key={tool.tool} tool={tool} />
      ))}
    </ul>
  );
}

/** The commands that will run, exactly; the password file is redacted. */
export function CommandList(props: {
  commands: readonly string[];
}): React.JSX.Element | null {
  if (!props.commands.length) return null;
  return (
    <ul className={styles.commands} aria-label="Commands">
      {props.commands.map((command, index) => (
        <li key={`${String(index)}:${command}`}>{command}</li>
      ))}
    </ul>
  );
}
