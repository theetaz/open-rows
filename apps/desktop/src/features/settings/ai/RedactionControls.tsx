// SPDX-License-Identifier: Apache-2.0
import type { RedactionRule } from '@openrows/ipc';
import { useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
/** Additional identifier/column rules; built-in sensitive-name rules always apply. */
export function RedactionControls(props: {
  rules: RedactionRule[];
  onChange: (rules: RedactionRule[]) => void;
}): React.JSX.Element {
  const [kind, setKind] = useState<'glob' | 'regex'>('glob');
  const [pattern, setPattern] = useState('');
  return (
    <section aria-label={t('redactionRules')} className={styles.form}>
      <p className={styles.notice}>{t('passwordTokenSecret')}</p>
      <SelectField
        label={t('ruleType')}
        value={kind}
        options={[
          { value: 'glob', label: t('glob') },
          { value: 'regex', label: t('regularExpression') },
        ]}
        onValueChange={setKind}
      />
      <TextField
        label={t('schemaTableColumn')}
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
        {t('addRedactionRule')}
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
              aria-label={t('removeRulePattern', { pattern: rule.pattern })}
              onClick={() => {
                props.onChange(
                  props.rules.filter((_, position) => position !== index),
                );
              }}
            >
              {t('remove')}
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}
