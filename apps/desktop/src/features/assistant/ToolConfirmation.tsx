// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon } from '@openrows/ui';
import styles from './Assistant.module.css';
import { confirmMcpTool } from '@openrows/ipc';
import type { AgentEvent } from '@openrows/ipc';
import { t } from './lazyMessages';
/** No tool executes until a pending native prompt receives an explicit user answer. */
export function ToolConfirmation(props: {
  prompt: Extract<AgentEvent, { type: 'confirmation' }>;
  disabled: boolean;
}): React.JSX.Element {
  const [resolved, setResolved] = useState(false);
  const [failure, setFailure] = useState('');
  const respond = async (allow: boolean): Promise<void> => {
    setResolved(true);
    try {
      await confirmMcpTool(props.prompt.id, allow);
    } catch {
      setFailure(t('confirmationExpiredCancelled'));
    }
  };
  return (
    <section aria-label={t('mcpToolConfirmation')} className={styles.confirm}>
      <Icon name="plug" />
      <div>
        <strong>
          {t('allowServerTool', {
            server: props.prompt.server,
            tool: props.prompt.tool,
          })}
        </strong>
        <pre aria-label={t('mcpToolArguments')}>{props.prompt.arguments}</pre>
        <div className={styles.row}>
          <Button
            size="sm"
            disabled={resolved || props.disabled}
            onClick={() => {
              void respond(false);
            }}
          >
            {t('denyTool')}
          </Button>
          <Button
            size="sm"
            disabled={resolved || props.disabled}
            onClick={() => {
              void respond(true);
            }}
          >
            {t('allowTool')}
          </Button>
        </div>
        {failure && <p role="alert">{failure}</p>}
      </div>
    </section>
  );
}
