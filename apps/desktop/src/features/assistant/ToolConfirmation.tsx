// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { confirmMcpTool } from '@openrows/ipc';
import type { AgentEvent } from '@openrows/ipc';
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
      setFailure('Confirmation expired or cancelled.');
    }
  };
  return (
    <section aria-label="MCP tool confirmation">
      <p>
        Allow {props.prompt.server} → {props.prompt.tool} for this session?
      </p>
      <pre aria-label="MCP tool arguments">{props.prompt.arguments}</pre>
      <Button
        disabled={resolved || props.disabled}
        onClick={() => {
          void respond(true);
        }}
      >
        Allow tool
      </Button>
      <Button
        disabled={resolved || props.disabled}
        onClick={() => {
          void respond(false);
        }}
      >
        Deny tool
      </Button>
      {failure && <p role="alert">{failure}</p>}
    </section>
  );
}
