// SPDX-License-Identifier: Apache-2.0
import { TextArea } from '@openrows/ui';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

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
      <span>Startup commands</span>
      <TextArea
        aria-label="Startup commands"
        value={draft.startupCommands}
        placeholder="SET search_path TO app, public;"
        spellCheck={false}
        onChange={(event) => {
          update({ startupCommands: event.target.value });
        }}
      />
      <small>
        Run in order after every connect, reconnects included. Separate
        statements with semicolons; a failing statement fails the connect.
      </small>
    </label>
  );
}
