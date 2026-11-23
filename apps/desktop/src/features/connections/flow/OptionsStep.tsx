// SPDX-License-Identifier: Apache-2.0
import type { SafetyDto } from '@openrows/ipc';
import { AiSharingDisclosure } from '../AiSharingDisclosure';
import { EnvironmentFields, HistoryFields } from '../EnvironmentFields';
import { SessionLimitsFields } from '../SessionLimitsFields';
import type { ConnectionForm } from '../useConnectionForm';
import { POLICY_DETAILS, POLICY_LABELS } from '../../security/policies';
import { useSecurity } from '../../security/useSecurity';
import { ChoiceCards } from './ChoiceCards';
import type { Card } from './ChoiceCards';
import type { FlowIconName } from './FlowIcon';
import { FormSection } from './FormSection';
import page from './Flow.module.css';
import { t } from './messages';

const POLICIES: readonly [SafetyDto, FlowIconName][] = [
  ['unrestricted', 'bolt'],
  ['confirmWrites', 'shield'],
  ['readOnly', 'lock'],
  ['confirmAll', 'shield'],
  ['authenticateWrites', 'key'],
  ['authenticateAll', 'key'],
];

/**
 * Step 4: environment (what it does to the chrome), the safety policy as
 * cards (Prod preselects Confirm writes), history, the advanced limits and
 * startup SQL, and, for a saved connection, its AI data sharing.
 */
export function OptionsStep(props: {
  form: ConnectionForm;
  focusSafety: boolean;
}): React.JSX.Element {
  const { form } = props;
  const security = useSecurity();
  const cards: Card<SafetyDto>[] = POLICIES.map(([value, icon]) => ({
    value,
    icon,
    title: POLICY_LABELS[value],
    detail: POLICY_DETAILS[value],
    disabled: value.startsWith('authenticate') && !security.available,
  }));
  return (
    <>
      <h2 className={page.title}>{t('optionsTitle')}</h2>
      <p className={page.lede}>{t('optionsLede')}</p>
      <FormSection title={t('environment')} icon="tag">
        <EnvironmentFields form={form} />
      </FormSection>
      <FormSection
        title={t('safetyPolicy')}
        icon="shield"
        bare
        note={
          security.notice
            ? t('safetyNotice', { notice: security.notice })
            : undefined
        }
      >
        <ChoiceCards
          label={t('safetyPolicy')}
          cards={cards}
          value={form.draft.safety}
          autoFocus={props.focusSafety}
          onChange={(safety) => {
            form.update({ safety });
          }}
        />
      </FormSection>
      <FormSection title={t('history')} icon="history">
        <HistoryFields form={form} />
      </FormSection>
      <FormSection title={t('advanced')} icon="settings">
        <SessionLimitsFields form={form} />
      </FormSection>
      {form.profileId && (
        <FormSection title={t('aiSharing')} icon="sparkles" bare>
          <AiSharingDisclosure
            id={form.profileId}
            production={form.draft.envKind === 'prod'}
          />
        </FormSection>
      )}
    </>
  );
}
