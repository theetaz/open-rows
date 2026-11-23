// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect } from 'react';
import { DeleteConnectionSheet } from './DeleteConnectionSheet';
import { PasswordPromptSheet } from './PasswordPromptSheet';
import type { Connections, SheetState } from './useConnections';

const ConnectionTransferSheet = lazy(() => import('./ConnectionTransferSheet'));

/**
 * The connection flow a new, new-file or edit request opens as a tab: a file
 * opened from Finder starts at Details, a safety change at Options.
 */
export type ConnectionFlowRequest = {
  mode: 'new' | 'edit';
  profileId?: string;
  engine?: 'sqlite' | 'duckdb';
  filePath?: string;
  focus?: 'safety';
};

/** The flow tab a sheet request becomes, or null for a real sheet. */
function flowRequest(sheet: SheetState): ConnectionFlowRequest | null {
  if (sheet?.kind === 'new') return { mode: 'new' };
  if (sheet?.kind === 'new-file')
    return {
      mode: 'new',
      engine: sheet.engine ?? 'sqlite',
      filePath: sheet.path,
    };
  if (sheet?.kind === 'edit')
    return {
      mode: 'edit',
      profileId: sheet.profile.id,
      ...(sheet.focus ? { focus: sheet.focus } : {}),
    };
  return null;
}

/** Hands new and edit requests to the flow tab; each is consumed once. */
function useFlowRouting(
  connections: Connections,
  onFlow: (request: ConnectionFlowRequest) => void,
): boolean {
  const { sheet, setSheet } = connections;
  useEffect(() => {
    const request = flowRequest(sheet);
    if (!request) return;
    onFlow(request);
    setSheet(null);
  }, [sheet, onFlow, setSheet]);
  return flowRequest(sheet) !== null;
}

/**
 * The connection sheet currently requested by the rail, header or keyboard;
 * a connect waiting for its password comes first. New and edit requests open
 * the connection flow tab through `onFlow`.
 */
export function ConnectionSheets({
  connections,
  onFlow,
}: {
  connections: Connections;
  onFlow: (request: ConnectionFlowRequest) => void;
}): React.JSX.Element | null {
  const { sheet, setSheet, passwordRequest } = connections;
  const routed = useFlowRouting(connections, onFlow);
  if (passwordRequest)
    return (
      <PasswordPromptSheet
        key={passwordRequest.profile.id}
        request={passwordRequest}
      />
    );
  if (!sheet || routed) return null;
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
  if (sheet.kind !== 'import' && sheet.kind !== 'export') return null;
  return (
    <Suspense fallback={null}>
      <ConnectionTransferSheet
        mode={sheet.kind}
        profiles={connections.profiles}
        ids={sheet.kind === 'export' ? sheet.ids : undefined}
        onClose={close}
        onImported={connections.reload}
      />
    </Suspense>
  );
}
