// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { cancelInvestigationSchedule } from '@openrows/ipc';
import type { ProfileDto, ScheduledJob } from '@openrows/ipc';
import { ArtifactPreview } from '../assistant/ArtifactPreview';
/** Review retained results; accepting a board still saves it paused. */
export function ScheduleResults(props: {
  profile: ProfileDto;
  database: string;
  jobs: ScheduledJob[];
  refresh: () => Promise<void>;
  setStatus: (value: string) => void;
}): React.JSX.Element {
  const { jobs, refresh, setStatus } = props;
  return (
    <>
      {jobs
        .filter(
          (job) =>
            job.connection === props.profile.id &&
            job.database === props.database,
        )
        .map((job) => (
          <section key={job.id} aria-label="Scheduled investigation">
            <p>
              {job.config.provider} / {job.config.model} ·{' '}
              {job.config.enabled ? 'enabled' : 'disabled'} ·{' '}
              {job.running ? 'running' : 'idle'}
            </p>
            {job.failure && <p role="alert">{job.failure}</p>}
            <Button
              onClick={() => {
                void cancelInvestigationSchedule(job.id)
                  .then(refresh)
                  .catch(() => {
                    setStatus('Cancellation failed.');
                  });
              }}
            >
              Cancel schedule
            </Button>
            {job.results.map((result, index) => (
              <details key={index}>
                <summary>
                  {result.audit.outcome} · ≤ {result.audit.tokens} tokens · ≤{' '}
                  {result.audit.costMicros} cost millionths
                </summary>
                {result.events.map((event, item) =>
                  event.type === 'artifact' ? (
                    <ArtifactPreview
                      key={item}
                      {...event}
                      connection={props.profile.id}
                      disabled={false}
                    />
                  ) : event.type === 'text' ? (
                    <p key={item}>Unverified interpretation: {event.delta}</p>
                  ) : event.type === 'trace' ? (
                    <p key={item}>
                      {event.trace.tool}: {event.trace.status}
                    </p>
                  ) : null,
                )}
              </details>
            ))}
          </section>
        ))}
    </>
  );
}
