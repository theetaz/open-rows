// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
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
    <section aria-label={t('mcpToolConfirmation')}>
      <p>
        {t('allowServerTool', {
          server: props.prompt.server,
          tool: props.prompt.tool,
        })}
      </p>
      <pre aria-label={t('mcpToolArguments')}>{props.prompt.arguments}</pre>
      <Button
        disabled={resolved || props.disabled}
        onClick={() => {
          void respond(true);
        }}
      >
        {t('allowTool')}
      </Button>
      <Button
        disabled={resolved || props.disabled}
        onClick={() => {
          void respond(false);
        }}
      >
        {t('denyTool')}
      </Button>
      {failure && <p role="alert">{failure}</p>}
    </section>
  );
}
