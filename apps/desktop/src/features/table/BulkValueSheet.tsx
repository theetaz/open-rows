// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { textValue } from './cellValue';
import type { CellValue } from './cellValue';
import styles from './Table.module.css';

/** "Set value for N cells…": one text value staged in every selected cell. */
export function BulkValueSheet({
  count,
  onApply,
  onClose,
}: {
  count: number;
  onApply: (value: CellValue) => void;
  onClose: () => void;
}): React.JSX.Element {
  const [text, setText] = useState('');
  const apply = (): void => {
    onApply(textValue(text));
    onClose();
  };
  return (
    <Sheet
      title={`Set value for ${count.toLocaleString('en-US')} cells`}
      size="alert"
      onClose={onClose}
    >
      <p className={styles.editNote}>
        The value is staged in every selected cell; each cell is checked against
        its column type first. Nothing is written until you commit.
      </p>
      <TextField
        aria-label="Value for the selected cells"
        data-initial-focus
        value={text}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
            event.preventDefault();
            apply();
          }
        }}
      />
      <SheetActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button variant="primary" onClick={apply}>
          Stage value
        </Button>
      </SheetActions>
    </Sheet>
  );
}
