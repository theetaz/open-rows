// SPDX-License-Identifier: Apache-2.0
import { TextArea } from '@openrows/ui';
import type { ConnectionForm } from './useConnectionForm';
import styles from './flow/Form.module.css';
import { t } from './messages/routes';

/**
 * Startup commands in the Advanced section: SQL run after every connect,
 * reconnects included, before the session's timeout settings are applied.
 */
export function StartupCommandsField({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, update } = form;
  return (
    <label className={styles.stacked}>
      <span>{t('startupCommands')}</span>
      <TextArea
        aria-label={t('startupCommands')}
        value={draft.startupCommands}
        placeholder={t('setSearchPath')}
        spellCheck={false}
        onChange={(event) => {
          update({ startupCommands: event.target.value });
        }}
      />
      <small>{t('runOrderAfter')}</small>
    </label>
  );
}
