// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { ProfileDto } from '@openrows/ipc';
import { deletePrompt } from './useConnections';

/**
 * Compact alert sheet used only when the native macOS alert is unavailable
 * (browser preview). Cancel is focused, so Return never deletes by accident.
 */
export function DeleteConnectionSheet(props: {
  profile: ProfileDto;
  onCancel: () => void;
  onDelete: (profile: ProfileDto) => void;
}): React.JSX.Element {
  const prompt = deletePrompt(props.profile);
  return (
    <Sheet title={prompt.title} size="alert" onClose={props.onCancel}>
      <p>{prompt.message}</p>
      <SheetActions>
        <Button onClick={props.onCancel}>Cancel</Button>
        <Button
          variant="destructive"
          onClick={() => {
            props.onDelete(props.profile);
          }}
        >
          {prompt.action}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
