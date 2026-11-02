// SPDX-License-Identifier: Apache-2.0
import { TextArea } from '@openrows/ui';
import { documentError } from './documentText';
import styles from './DocumentField.module.css';

/**
 * The whole document as JSON/EJSON (FR-DATA-07). The text must be one object;
 * BSON type and range validation runs natively before review.
 */
export function DocumentField(props: {
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}): React.JSX.Element {
  const error = documentError(props.value);
  return (
    <>
      <TextArea
        aria-label="Edit __document"
        className={styles.documentField}
        spellCheck={false}
        value={props.value}
        disabled={props.disabled}
        aria-invalid={Boolean(error)}
        onChange={(event) => {
          props.onChange(event.target.value);
        }}
      />
      {error && !props.disabled && (
        <p className={styles.documentError} role="alert">
          {error}
        </p>
      )}
    </>
  );
}
