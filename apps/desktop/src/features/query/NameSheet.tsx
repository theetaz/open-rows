// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Ask for a name (Add to Queries, Rename…). The sheet stays open with the
 * error when `onSubmit` rejects.
 */
export function NameSheet(props: {
  title: string;
  label: string;
  initial: string;
  action: string;
  onSubmit: (name: string) => Promise<void>;
  onClose: () => void;
}): React.JSX.Element {
  const [name, setName] = useState(props.initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const submit = (): void => {
    if (!name.trim() || busy) return;
    setBusy(true);
    props.onSubmit(name.trim()).then(props.onClose, (failure: unknown) => {
      setBusy(false);
      setError(message(failure));
    });
  };
  return (
    <Sheet title={props.title} size="alert" onClose={props.onClose}>
      <TextField
        label={props.label}
        data-initial-focus
        value={name}
        {...(error ? { error } : {})}
        onChange={(event) => {
          setName(event.target.value);
          setError(null);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
            event.preventDefault();
            submit();
          }
        }}
      />
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!name.trim() || busy}
          onClick={submit}
        >
          {props.action}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
