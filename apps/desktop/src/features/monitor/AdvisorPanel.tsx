// SPDX-License-Identifier: Apache-2.0
import type { MonitorTarget } from '@openrows/ipc';
import { useAdvisor } from './useAdvisor';
import { AdvisorList } from './AdvisorList';

/** Connection-wide deterministic maintenance and query proposals. */
export function AdvisorPanel(props: {
  target: MonitorTarget;
  revision: string;
  now: number;
  production: boolean;
  onEvidence: (link: string) => void;
}): React.JSX.Element {
  const advisor = useAdvisor(props.target, props.revision);
  return (
    <>
      {advisor.error && <p role="status">{advisor.error}</p>}
      <AdvisorList
        key={`${props.target.profileId}:${props.target.database}`}
        {...props}
        suggestions={advisor.suggestions}
      />
    </>
  );
}
