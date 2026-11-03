// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { textValue } from './cellValue';
import type { CellValue } from './cellValue';
import styles from './Table.module.css';
import { t } from './messages/lazyData';

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
      title={t('setValueCount', { count: count.toLocaleString('en-US') })}
      size="alert"
      onClose={onClose}
    >
      <p className={styles.editNote}>{t('valueStagedEvery')}</p>
      <TextField
        aria-label={t('valueSelectedCells')}
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
        <Button onClick={onClose}>{t('cancel')}</Button>
        <Button variant="primary" onClick={apply}>
          {t('stageValue')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
