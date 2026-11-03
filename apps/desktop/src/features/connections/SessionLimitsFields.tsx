// SPDX-License-Identifier: Apache-2.0
import { DisclosureSection, SelectField } from '@openrows/ui';
import { generalSettings } from '../settings/general';
import { isFileEngine } from './engines';
import { idleOptions, limitsSummary, statementOptions } from './limitsForm';
import { StartupCommandsField } from './StartupCommandsField';
import type { LimitsDraft } from './limitsForm';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/routes';

/**
 * "Advanced" disclosure: the statement timeout (cancelled and reported as a
 * timeout), for servers the idle timeout after which the connection closes
 * until the next query, and the startup commands. Collapsed until a limit or
 * a startup command is set.
 */
export function SessionLimitsFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { limits, engine } = form.draft;
  const { statementTimeoutSeconds } = generalSettings.useValue();
  const update = (patch: Partial<LimitsDraft>): void => {
    form.update({ limits: { ...limits, ...patch } });
  };
  const summary = limitsSummary(limits);
  const startup = form.draft.startupCommands.trim() !== '';
  return (
    <DisclosureSection
      label={
        startup
          ? t('advancedSummaryStartup', { summary })
          : t('advancedSummary', { summary })
      }
      defaultOpen={summary !== t('defaults') || startup}
    >
      <div className={styles.section}>
        <SelectField
          className={styles.row}
          label={t('queryTimeout')}
          aria-label={t('statementTimeout')}
          value={limits.statementTimeout}
          options={statementOptions(
            limits.statementTimeout,
            statementTimeoutSeconds,
          )}
          onValueChange={(statementTimeout) => {
            update({ statementTimeout });
          }}
        />
        <small className={styles.hint}>{t('statementRunningLonger')}</small>
        {!isFileEngine(engine) && (
          <>
            <SelectField
              className={styles.row}
              label={t('closeIfIdle')}
              aria-label={t('idleTimeout')}
              value={limits.idleTimeout}
              options={idleOptions(limits.idleTimeout)}
              onValueChange={(idleTimeout) => {
                update({ idleTimeout });
              }}
            />
            <small className={styles.hint}>{t('disconnectsAfterLong')}</small>
          </>
        )}
        <StartupCommandsField form={form} />
      </div>
    </DisclosureSection>
  );
}
