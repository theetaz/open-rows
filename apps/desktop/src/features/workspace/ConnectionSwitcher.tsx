// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Badge } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { Connections } from '../connections';
import { routeStatus } from './routeStatus';

/** Preview the target connection's environment and safety before connecting. */
export function ConnectionSwitcher(props: {
  connections: Connections;
  onClose: () => void;
}): React.JSX.Element {
  const { connections } = props;
  const [id, setId] = useState(
    connections.active?.id ?? connections.profiles[0]?.id ?? '',
  );
  const profile = connections.profiles.find((item) => item.id === id);
  return (
    <Sheet title="Switch connection" size="alert" onClose={props.onClose}>
      <label>
        Connection
        <select
          data-initial-focus
          value={id}
          onChange={(event) => {
            setId(event.target.value);
          }}
        >
          {connections.profiles.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </label>
      {profile && (
        <div role="group" aria-label="Connection preview">
          <Badge style={{ borderColor: profile.env.color }}>
            {profile.env.label ?? profile.env.kind}
          </Badge>
          <p>
            {profile.safety.readOnly ? 'Read-only' : 'Read/write'} ·{' '}
            {profile.safety.confirmWrites
              ? 'Confirm writes'
              : 'Writes without confirmation'}
          </p>
          {profile.env.kind === 'prod' && <p>Production connection</p>}
          <p>{routeStatus(profile)}</p>
          <p>
            Existing tabs stay open. Switching explicitly connects this profile.
          </p>
        </div>
      )}
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!profile}
          onClick={() => {
            connections.select(id);
            props.onClose();
          }}
        >
          Switch connection
        </Button>
      </SheetActions>
    </Sheet>
  );
}
