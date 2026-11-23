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
import parts from './MonitorParts.module.css';
import { t } from './messages/lazyInvestigation';
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
            aria-label={t('scheduledInvestigation')}
            className={parts.schedule}
          >
            <p>
              {t('providerModelJob', {
                provider: job.config.provider,
                model: job.config.model,
                job: stateLabel(job),
                runs: job.runs,
                maxRuns: job.config.maxRuns,
              })}
            </p>
            {job.failure && <p role="alert">{job.failure}</p>}
            <JobActions job={job} {...props} />
            {job.results.map((result, index) => (
              <details key={index}>
                <summary>
                  {t('outcomeTokensTokens', {
                    outcome: result.audit.outcome,
                    tokens: result.audit.tokens,
                    costMicros: result.audit.costMicros,
                  })}
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
      return t('waitingNextInterval');
    case 'running':
      return t('running');
    case 'finished':
      return t('finished');
    case 'stopped':
      return t('stopped');
    case 'paused':
      if (job.pause === 'locked') return t('pausedWhileApp');
      if (job.pause === 'aiOff') return t('pausedWhileAi');
      return t('paused');
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
    <div className={parts.scheduleActions}>
      <Button
        size="sm"
        disabled={job.state !== 'waiting'}
        onClick={() => {
          act(runInvestigationSchedule(job.id), t('runFailedStart'));
        }}
      >
        {t('runNow')}
      </Button>
      <Button
        size="sm"
        disabled={ended}
        onClick={() => {
          act(
            pauseInvestigationSchedule(job.id, !paused),
            paused ? t('resumeFailed') : t('pauseFailed'),
          );
        }}
      >
        {paused ? t('resumeSchedule') : t('pauseSchedule')}
      </Button>
      <Button
        size="sm"
        onClick={() => {
          act(cancelInvestigationSchedule(job.id), t('cancellationFailed'));
        }}
      >
        {ended ? t('removeSchedule') : t('cancelSchedule')}
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
      return (
        <p>{t('unverifiedInterpretationDelta', { delta: event.delta })}</p>
      );
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
