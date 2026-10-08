// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { EngineDto } from '@openrows/ipc';
import { ConnectionSheet } from './ConnectionSheet';
import { DeleteConnectionSheet } from './DeleteConnectionSheet';
import { EnginePicker } from './EnginePicker';
import type { Connections } from './useConnections';

/** The connection sheet currently requested by the rail, header or keyboard. */
export function ConnectionSheets({
  connections,
}: {
  connections: Connections;
}): React.JSX.Element | null {
  const { sheet, setSheet } = connections;
  if (!sheet) return null;
  const close = (): void => {
    setSheet(null);
  };
  if (sheet.kind === 'delete')
    return (
      <DeleteConnectionSheet
        profile={sheet.profile}
        onCancel={close}
        onDelete={(profile) => void connections.remove(profile)}
      />
    );
  if (sheet.kind === 'new')
    return <NewConnection connections={connections} onClose={close} />;
  return (
    <ConnectionSheet
      key={sheet.profile.id}
      profile={sheet.profile}
      onClose={close}
      onSaved={connections.saved}
      onDelete={connections.confirmRemove}
      onDuplicate={(target) => void connections.duplicate(target)}
    />
  );
}

/** New connection: pick the engine (TablePlus), then fill in its form. */
function NewConnection(props: {
  connections: Connections;
  onClose: () => void;
}): React.JSX.Element {
  const [engine, setEngine] = useState<EngineDto | null>(null);
  const { connections } = props;
  if (!engine)
    return <EnginePicker onCancel={props.onClose} onCreate={setEngine} />;
  return (
    <ConnectionSheet
      engine={engine}
      onClose={props.onClose}
      onSaved={connections.saved}
      onDelete={connections.confirmRemove}
      onDuplicate={(target) => void connections.duplicate(target)}
    />
  );
}
