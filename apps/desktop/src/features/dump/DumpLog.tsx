// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { cancelDump } from '@openrows/ipc';
import { elapsedText, useElapsed } from '../runfile';
import { showDump, useDumpRun } from './dumpStore';
import type { DumpRun } from './dumpStore';
import { outcomeText, progressFraction, progressText } from './dumpJob';
import { CommandList } from './ToolList';
import styles from './Dump.module.css';

function Running(props: { run: DumpRun; elapsed: number }): React.JSX.Element {
  const fraction = progressFraction(props.run);
  return (
    <div className={styles.status} role="status">
      <progress aria-label="Progress" max={1} value={fraction} />
      <p className={styles.counts}>
        <span>{progressText(props.run)}</span>
        <span>{elapsedText(props.elapsed)}</span>
      </p>
    </div>
  );
}

function Summary({ run }: { run: DumpRun }): React.JSX.Element {
  const failed = !!run.failure || run.finished?.outcome === 'failed';
  const ms = run.finished?.elapsedMs;
  const took = typeof ms === 'number' ? ` in ${elapsedText(ms)}` : '';
  const output = run.finished?.output ? ` · ${run.finished.output}` : '';
  return (
    <p
      className={failed ? styles.error : styles.summary}
      role={failed ? 'alert' : 'status'}
    >
      {run.failure ?? `${outcomeText(run)}${took}${output}`}
    </p>
  );
}

/** The tool's stderr, kept scrolled to the newest line. */
function Log({ lines }: { lines: readonly string[] }): React.JSX.Element {
  const ref = useRef<HTMLPreElement>(null);
  useEffect(() => {
    const element = ref.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [lines]);
  return (
    <pre ref={ref} className={styles.log} aria-label="Log" tabIndex={0}>
      {lines.length ? lines.join('\n') : 'No output yet.'}
    </pre>
  );
}

/** A started backup or restore: progress, the commands and the tool's log. */
export function DumpLog({ id }: { id: string }): React.JSX.Element | null {
  const run = useDumpRun(id);
  const running = !!run && !run.finished && !run.failure;
  const elapsed = useElapsed(run?.startedAt ?? 0, running);
  if (!run) return null;
  const close = (): void => {
    showDump(null);
  };
  const jobId = run.jobId;
  return (
    <Sheet title={run.title} onClose={close}>
      {running ? (
        <Running run={run} elapsed={elapsed} />
      ) : (
        <Summary run={run} />
      )}
      <CommandList commands={run.commands} />
      <Log lines={run.log} />
      <SheetActions>
        {running && (
          <Button
            disabled={!jobId}
            onClick={() => {
              if (jobId) void cancelDump(jobId).catch(() => undefined);
            }}
          >
            Cancel {run.kind === 'backup' ? 'Backup' : 'Restore'}
          </Button>
        )}
        <Button variant="primary" onClick={close}>
          {running ? 'Run in Background' : 'Done'}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
