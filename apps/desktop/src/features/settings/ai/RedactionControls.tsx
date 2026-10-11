// SPDX-License-Identifier: Apache-2.0
import type { RedactionRule } from '@openrows/ipc';
import { useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import styles from './Ai.module.css';
/** Additional identifier/column rules; built-in sensitive-name rules always apply. */
export function RedactionControls(props: {
  rules: RedactionRule[];
  onChange: (rules: RedactionRule[]) => void;
}): React.JSX.Element {
  const [kind, setKind] = useState<'glob' | 'regex'>('glob');
  const [pattern, setPattern] = useState('');
  return (
    <section aria-label="Redaction rules" className={styles.form}>
      <p className={styles.notice}>
        Password, token, secret, SSN, email, phone, card and key fields are
        always redacted. SQL literals and comments are removed at every sharing
        level.
      </p>
      <SelectField
        label="Rule type"
        value={kind}
        options={[
          { value: 'glob', label: 'Glob' },
          { value: 'regex', label: 'Regular expression' },
        ]}
        onValueChange={setKind}
      />
      <TextField
        label="Schema.table.column pattern"
        value={pattern}
        maxLength={512}
        onChange={(event) => {
          setPattern(event.target.value);
        }}
      />
      <Button
        type="button"
        disabled={!pattern || props.rules.length >= 32}
        onClick={() => {
          props.onChange([...props.rules, { kind, pattern }]);
          setPattern('');
        }}
      >
        Add redaction rule
      </Button>
      <ul>
        {props.rules.map((rule, index) => (
          <li key={`${String(index)}-${rule.pattern}`}>
            <code>
              {rule.kind}: {rule.pattern}
            </code>{' '}
            <Button
              size="sm"
              type="button"
              aria-label={`Remove rule ${rule.pattern}`}
              onClick={() => {
                props.onChange(
                  props.rules.filter((_, position) => position !== index),
                );
              }}
            >
              Remove
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}
