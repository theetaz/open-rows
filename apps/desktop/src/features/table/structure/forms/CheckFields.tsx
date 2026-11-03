// SPDX-License-Identifier: Apache-2.0
import { TextArea, TextField } from '@openrows/ui';
import type { NewCheckDto } from '@openrows/ipc';
import styles from './Forms.module.css';
import { t } from '../../messages/initial';

/** Why the check cannot be staged yet, or `null`. */
export function checkProblem(check: NewCheckDto): string | null {
  return check.expression.trim() ? null : t('writeConditionRows');
}

/** The check as sent: a blank name is none. */
export function cleanCheck(check: NewCheckDto): NewCheckDto {
  return {
    name: check.name?.trim() || null,
    expression: check.expression.trim(),
  };
}

/** Name and the boolean condition of a check constraint. */
export function CheckFields({
  check,
  onChange,
}: {
  check: NewCheckDto;
  onChange: (check: NewCheckDto) => void;
}): React.JSX.Element {
  return (
    <div className={styles.grid}>
      <TextField
        label={t('name')}
        className={styles.wide}
        value={check.name ?? ''}
        placeholder={t('automatic')}
        autoComplete="off"
        onChange={(event) => {
          onChange({ ...check, name: event.target.value });
        }}
      />
      <div className={styles.wide}>
        <TextArea
          aria-label={t('condition')}
          placeholder={t('price0')}
          value={check.expression}
          spellCheck={false}
          onChange={(event) => {
            onChange({ ...check, expression: event.target.value });
          }}
        />
      </div>
    </div>
  );
}
