// SPDX-License-Identifier: Apache-2.0
import { ConnectionSheet } from './ConnectionSheet';
import { DeleteConnectionSheet } from './DeleteConnectionSheet';
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
  const profile = sheet.kind === 'edit' ? sheet.profile : undefined;
  return (
    <ConnectionSheet
      key={profile?.id ?? 'new'}
      profile={profile}
      onClose={close}
      onSaved={connections.saved}
      onDelete={connections.confirmRemove}
      onDuplicate={(target) => void connections.duplicate(target)}
    />
  );
}
