// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef } from 'react';
import type { RefObject } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '@openrows/ui';
import { clearEndedJobs, removeJob } from './jobStore';
import type { Job } from './jobStore';
import styles from './Jobs.module.css';

/** Open or reveal a job's output; failures are shown on the job's row. */
type OutputActions = {
  open: (path: string) => Promise<void>;
  reveal: (path: string) => Promise<void>;
};

type Props = {
  jobs: readonly Job[];
  opener: RefObject<HTMLButtonElement | null>;
  output: OutputActions;
  onClose: () => void;
};

/** Below the opener, right-aligned to it and inside the window. */
function place(panel: HTMLElement | null, opener: HTMLElement | null): void {
  if (!panel || !opener) return;
  const anchor = opener.getBoundingClientRect();
  const left = Math.max(
    8,
    Math.min(
      anchor.right - panel.offsetWidth,
      innerWidth - panel.offsetWidth - 8,
    ),
  );
  panel.style.setProperty('left', `${String(left)}px`);
  panel.style.setProperty('top', `${String(anchor.bottom + 6)}px`);
}

/** Close on Escape and on a press outside the popover and its opener. */
function useDismiss(
  panel: RefObject<HTMLDivElement | null>,
  opener: RefObject<HTMLButtonElement | null>,
  onClose: () => void,
): void {
  useEffect(() => {
    const outside = (event: Event): void => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (panel.current?.contains(target) || opener.current?.contains(target))
        return;
      onClose();
    };
    const escape = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') onClose();
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [panel, opener, onClose]);
}

/**
 * Background jobs of every kind with progress, Cancel, Open and Show in
 * Finder, like Safari's downloads popover.
 */
export function JobsPopover(props: Props): React.JSX.Element {
  const panel = useRef<HTMLDivElement>(null);
  const { opener, onClose } = props;
  useLayoutEffect(() => {
    place(panel.current, opener.current);
  });
  useDismiss(panel, opener, onClose);
  const ended = props.jobs.some((job) => job.status !== 'running');
  return createPortal(
    <div
      ref={panel}
      className={styles.anchor}
      role="dialog"
      aria-label="Background jobs"
    >
      <div className={styles.panel}>
        <div className={styles.header}>
          <span>Background Jobs</span>
          <Button
            size="sm"
            variant="ghost"
            disabled={!ended}
            onClick={clearEndedJobs}
          >
            Clear
          </Button>
        </div>
        {props.jobs.length === 0 ? (
          <p className={styles.empty}>
            Exports and SQL file runs appear here while they run.
          </p>
        ) : (
          <ul className={styles.list}>
            {props.jobs.map((job) => (
              <JobRow key={job.id} job={job} output={props.output} />
            ))}
          </ul>
        )}
      </div>
    </div>,
    document.body,
  );
}

function JobRow({
  job,
  output,
}: {
  job: Job;
  output: OutputActions;
}): React.JSX.Element {
  const running = job.status === 'running';
  return (
    <li className={styles.job} data-status={job.status} aria-label={job.title}>
      <div className={styles.titleRow}>
        <span className={styles.title} title={job.title}>
          {job.title}
        </span>
        <span className={styles.kind}>{kindLabel(job.kind)}</span>
      </div>
      {running && (
        <progress
          aria-label={`${job.title} progress`}
          {...(job.fraction === undefined ? {} : { value: job.fraction })}
        />
      )}
      <p className={styles.detail} role={running ? undefined : 'status'}>
        {job.detail}
      </p>
      <JobActions job={job} output={output} />
    </li>
  );
}

function kindLabel(kind: string): string {
  if (kind === 'run-file') return 'SQL file';
  if (kind === 'export') return 'Export';
  if (kind === 'import') return 'Import';
  return kind;
}

function JobActions({
  job,
  output,
}: {
  job: Job;
  output: OutputActions;
}): React.JSX.Element {
  const path = job.status === 'finished' ? job.output : undefined;
  return (
    <div className={styles.actions}>
      {job.details && (
        <Button size="sm" onClick={job.details}>
          Details…
        </Button>
      )}
      {path && (
        <>
          <Button
            size="sm"
            onClick={() => void output.open(path).catch(() => undefined)}
          >
            Open
          </Button>
          <Button
            size="sm"
            onClick={() => void output.reveal(path).catch(() => undefined)}
          >
            Show in Finder
          </Button>
        </>
      )}
      {job.status === 'running' ? (
        <Button size="sm" disabled={!job.cancel} onClick={job.cancel}>
          Cancel
        </Button>
      ) : (
        <Button
          size="sm"
          variant="ghost"
          aria-label={`Remove ${job.title}`}
          onClick={() => {
            removeJob(job.id);
          }}
        >
          Remove
        </Button>
      )}
    </div>
  );
}
