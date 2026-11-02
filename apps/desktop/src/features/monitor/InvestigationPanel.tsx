// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { AssistantDock } from '../assistant/AssistantDock';
import { toggleAssistant, useConversation } from '../assistant/conversation';
import { InvestigationAudit } from './InvestigationAudit';
import { InvestigationSchedulePane } from './InvestigationSchedulePane';
import styles from './Monitor.module.css';
/** Monitoring assistant is user-triggered; mounting never calls a model. */
export function InvestigationPanel(props: {
  profile: ProfileDto;
  database: string;
  connected: boolean;
  onOpen: (sql: string) => void;
}): React.JSX.Element {
  const tabId = `monitor:${props.profile.id}:${props.database}`;
  const conversation = useConversation(tabId);
  return (
    <div className={styles.investigation}>
      <p className={styles.note}>
        Ask about health, slow queries, blocking locks or regressions. Findings
        cite the evidence ids they came from. Plain model text is an unverified
        interpretation.
      </p>
      {!conversation.open && (
        <Button
          onClick={() => {
            toggleAssistant(tabId);
          }}
        >
          Open assistant
        </Button>
      )}
      <AssistantDock
        tabId={tabId}
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
    </div>
  );
}
