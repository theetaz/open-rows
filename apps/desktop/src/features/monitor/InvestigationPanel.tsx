// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';
import { AssistantDock } from '../assistant/AssistantDock';
import { InvestigationAudit } from './InvestigationAudit';
import { InvestigationSchedulePane } from './InvestigationSchedulePane';
/** Monitoring assistant is user-triggered; mounting never calls a model. */
export function InvestigationPanel(props: {
  profile: ProfileDto;
  database: string;
  connected: boolean;
  onOpen: (sql: string) => void;
}): React.JSX.Element {
  return (
    <>
      <p>
        Ask about health, query statistics, blocking or regressions. Findings
        cite authorised sample ids. Plain model text is an unverified
        interpretation.
      </p>
      <AssistantDock
        tabId={`monitor:${props.profile.id}:${props.database}`}
        {...props}
        monitoring
        sql={() => ''}
        lastError={null}
        plan={null}
        onInsert={props.onOpen}
      />
      <InvestigationAudit connection={props.profile.id} />
      <InvestigationSchedulePane
        profile={props.profile}
        database={props.database}
      />
    </>
  );
}
