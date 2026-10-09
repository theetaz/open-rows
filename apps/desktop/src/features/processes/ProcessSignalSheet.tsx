// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { signalPrompt } from './processSignals';
import type { PendingSignal } from './processSignals';

/**
 * Compact alert used only when the native macOS alert is unavailable
 * (browser preview). Cancel is focused, so Return never kills by accident.
 */
export function ProcessSignalSheet(props: {
  pending: PendingSignal;
  onCancel: () => void;
  onConfirm: () => void;
}): React.JSX.Element {
  const prompt = signalPrompt(props.pending);
  return (
    <Sheet title={prompt.title} size="alert" onClose={props.onCancel}>
      <p>{prompt.message}</p>
      <SheetActions>
        <Button onClick={props.onCancel}>Cancel</Button>
        <Button variant="destructive" onClick={props.onConfirm}>
          {prompt.action}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
