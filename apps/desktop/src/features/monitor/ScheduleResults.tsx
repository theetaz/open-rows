// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import {
  cancelInvestigationSchedule,
  pauseInvestigationSchedule,
  runInvestigationSchedule,
} from '@openrows/ipc';
import type { ProfileDto, ScheduledJob } from '@openrows/ipc';
import {
  ArtifactPreview,
  ArtifactRejected,
} from '../assistant/ArtifactPreview';
import styles from './Monitor.module.css';
type Props = {
  profile: ProfileDto;
  database: string;
  jobs: ScheduledJob[];
  refresh: () => Promise<void>;
  setStatus: (value: string) => void;
};
/** Review retained results; accepting a board still saves it paused. */
export function ScheduleResults(props: Props): React.JSX.Element {
  return (
    <>
      {props.jobs
        .filter(
          (job) =>
            job.connection === props.profile.id &&
            job.database === props.database,
        )
        .map((job) => (
          <section
            key={job.id}
            aria-label="Scheduled investigation"
            className={styles.schedule}
          >
            <p>
              {job.config.provider} / {job.config.model} · {stateLabel(job)} ·{' '}
              {job.runs}/{job.config.maxRuns} runs
            </p>
            {job.failure && <p role="alert">{job.failure}</p>}
            <JobActions job={job} {...props} />
            {job.results.map((result, index) => (
              <details key={index}>
                <summary>
                  {result.audit.outcome} · ≤ {result.audit.tokens} tokens · ≤{' '}
                  {result.audit.costMicros} cost millionths
                </summary>
                {result.events.map((event, item) => (
                  <ResultEvent
                    key={item}
                    event={event}
                    connection={props.profile.id}
                  />
                ))}
              </details>
            ))}
          </section>
        ))}
    </>
  );
}
function stateLabel(job: ScheduledJob): string {
  switch (job.state) {
    case 'waiting':
      return 'Waiting for the next interval';
    case 'running':
      return 'Running';
    case 'finished':
      return 'Finished';
    case 'stopped':
      return 'Stopped';
    case 'paused':
      if (job.pause === 'locked') return 'Paused while the app is locked';
      if (job.pause === 'aiOff') return 'Paused while AI is off';
      return 'Paused';
  }
}
function JobActions(props: Props & { job: ScheduledJob }): React.JSX.Element {
  const { job, refresh, setStatus } = props;
  const act = (work: Promise<void>, failure: string): void => {
    void work.then(refresh).catch((error: unknown) => {
      setStatus(error instanceof Error ? error.message : failure);
    });
  };
  const ended = job.state === 'finished' || job.state === 'stopped';
  const paused = job.pause === 'user';
  return (
    <div className={styles.scheduleActions}>
      <Button
        size="sm"
        disabled={job.state !== 'waiting'}
        onClick={() => {
          act(runInvestigationSchedule(job.id), 'Run failed to start.');
        }}
      >
        Run now
      </Button>
      <Button
        size="sm"
        disabled={ended}
        onClick={() => {
          act(
            pauseInvestigationSchedule(job.id, !paused),
            paused ? 'Resume failed.' : 'Pause failed.',
          );
        }}
      >
        {paused ? 'Resume schedule' : 'Pause schedule'}
      </Button>
      <Button
        size="sm"
        onClick={() => {
          act(cancelInvestigationSchedule(job.id), 'Cancellation failed.');
        }}
      >
        {ended ? 'Remove schedule' : 'Cancel schedule'}
      </Button>
    </div>
  );
}
function ResultEvent(props: {
  event: ScheduledJob['results'][number]['events'][number];
  connection: string;
}): React.JSX.Element | null {
  const { event } = props;
  switch (event.type) {
    case 'artifact':
      return (
        <ArtifactPreview
          {...event}
          connection={props.connection}
          disabled={false}
        />
      );
    case 'artifactRejected':
      return <ArtifactRejected {...event} />;
    case 'text':
      return <p>Unverified interpretation: {event.delta}</p>;
    case 'trace':
      return (
        <p>
          {event.trace.tool}: {event.trace.status}
        </p>
      );
    default:
      return null;
  }
}
