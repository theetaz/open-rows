// SPDX-License-Identifier: Apache-2.0
import { DisclosureSection, SelectField } from '@openrows/ui';
import { generalSettings } from '../settings/general';
import { isFileEngine } from './engines';
import { idleOptions, limitsSummary, statementOptions } from './limitsForm';
import type { LimitsDraft } from './limitsForm';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

/**
 * "Advanced" disclosure: the statement timeout (cancelled and reported as a
 * timeout) and, for servers, the idle timeout after which the connection
 * closes until the next query. Collapsed until a limit is set.
 */
export function SessionLimitsFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { limits, engine } = form.draft;
  const { statementTimeoutSeconds } = generalSettings.use();
  const update = (patch: Partial<LimitsDraft>): void => {
    form.update({ limits: { ...limits, ...patch } });
  };
  const summary = limitsSummary(limits);
  return (
    <DisclosureSection
      label={`Advanced · ${summary}`}
      defaultOpen={summary !== 'Defaults'}
    >
      <div className={styles.section}>
        <SelectField
          className={styles.row}
          label="Query timeout"
          aria-label="Statement timeout"
          value={limits.statementTimeout}
          options={statementOptions(
            limits.statementTimeout,
            statementTimeoutSeconds,
          )}
          onValueChange={(statementTimeout) => {
            update({ statementTimeout });
          }}
        />
        <small className={styles.hint}>
          A statement running longer is cancelled. The default is set in
          Settings ▸ General.
        </small>
        {!isFileEngine(engine) && (
          <>
            <SelectField
              className={styles.row}
              label="Close if idle"
              aria-label="Idle timeout"
              value={limits.idleTimeout}
              options={idleOptions(limits.idleTimeout)}
              onValueChange={(idleTimeout) => {
                update({ idleTimeout });
              }}
            />
            <small className={styles.hint}>
              Disconnects after this long without a query and reconnects on next
              use. Never while a transaction or staged edits are open.
            </small>
          </>
        )}
      </div>
    </DisclosureSection>
  );
}
