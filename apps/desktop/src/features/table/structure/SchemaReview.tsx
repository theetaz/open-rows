// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import { ReviewSheet } from '../ReviewSheet';
import type { SchemaEditing } from './useSchemaEditing';
import styles from './Structure.module.css';

/** The generated DDL, its warnings and (when required) the typed table name. */
export function SchemaReview({
  schema,
}: {
  schema: SchemaEditing;
}): React.JSX.Element | null {
  const [typed, setTyped] = useState('');
  const review = schema.review;
  if (!review) return null;
  const count = schema.history.current.length;
  const needsName = review.confirmName !== null;
  return (
    <ReviewSheet
      title="Review structure changes"
      summary={`${String(count)} staged structure ${count === 1 ? 'change' : 'changes'}. The statements run in one transaction: any error rolls every one of them back.`}
      sql={review.statements.map((statement) => `${statement};`).join('\n')}
      error={schema.error}
      busy={schema.busy}
      blocked={
        Boolean(schema.history.uncertain) ||
        (needsName && typed !== review.confirmName)
      }
      onCancel={schema.close}
      onCommit={() => {
        schema.commit(review.token, needsName ? typed : undefined);
      }}
    >
      {review.warnings.length > 0 && (
        <ul className={styles.warnings} aria-label="Warnings">
          {review.warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
      )}
      {needsName && (
        <TextField
          label={`This connection confirms destructive changes. Type ${review.confirmName ?? ''} to commit.`}
          value={typed}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => {
            setTyped(event.target.value);
          }}
        />
      )}
    </ReviewSheet>
  );
}
