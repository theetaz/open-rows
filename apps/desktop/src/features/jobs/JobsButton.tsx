// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useRef } from 'react';
import { Icon, IconButton } from '@openrows/ui';
import { openExport, revealExport } from '@openrows/ipc';
import { setJobsOpen, useJobs, useJobsOpen } from './jobStore';
import styles from './Jobs.module.css';
const JobsPopover = lazy(() =>
  import('./JobsPopover').then((module) => ({ default: module.JobsPopover })),
);

const output = { open: openExport, reveal: revealExport };

/**
 * Toolbar button of the background jobs popover; its badge counts running
 * jobs. Show Background Jobs opens it too.
 */
export function JobsButton(): React.JSX.Element {
  const jobs = useJobs();
  const open = useJobsOpen();
  const opener = useRef<HTMLButtonElement>(null);
  const running = jobs.filter((job) => job.status === 'running').length;
  const label = running
    ? `Background jobs (${String(running)} running)`
    : 'Background jobs';
  return (
    <>
      <IconButton
        ref={opener}
        aria-label={label}
        title={label}
        aria-expanded={open}
        aria-haspopup="dialog"
        variant="ghost"
        size="sm"
        className={styles.button}
        onClick={() => {
          setJobsOpen(!open);
        }}
      >
        <Icon name="jobs" />
        {running > 0 && (
          <span className={styles.badge} aria-hidden="true">
            {running}
          </span>
        )}
      </IconButton>
      {open && (
        <Suspense fallback={null}>
          <JobsPopover
            jobs={jobs}
            opener={opener}
            output={output}
            onClose={() => {
              setJobsOpen(false);
            }}
          />
        </Suspense>
      )}
    </>
  );
}
