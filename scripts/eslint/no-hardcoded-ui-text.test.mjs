// SPDX-License-Identifier: Apache-2.0
import { describe, it } from 'node:test';
import { RuleTester } from 'eslint';
import tseslint from 'typescript-eslint';
import { noHardcodedUiText } from './no-hardcoded-ui-text.mjs';

RuleTester.describe = describe;
RuleTester.it = it;
RuleTester.itOnly = it.only;

const tester = new RuleTester({
  languageOptions: {
    parser: tseslint.parser,
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

const literal = [{ messageId: 'literal' }];

tester.run('no-hardcoded-ui-text', noHardcodedUiText, {
  valid: [
    "<p>{t('saved')}</p>",
    '<p>{count}</p>',
    '<p> · </p>',
    "<p>{' '}</p>",
    '<code>SELECT 1</code>',
    "<pre>{'SELECT 1'}</pre>",
    '<input type="text" name="host" />',
    "<input aria-label={t('host')} placeholder={t('hostHint')} />",
    "const item = { label: t('save'), value: 'save' };",
    "const icon = { kind: 'Table' };",
    { code: '<Badge>OpenRows</Badge>', options: [{ allow: ['OpenRows'] }] },
    { code: '<Icon title="OpenRows" />', options: [{ allow: ['OpenRows'] }] },
  ],
  invalid: [
    { code: '<p>Saved</p>', errors: literal },
    { code: '<p>Saved {name}</p>', errors: literal },
    { code: "<p>{'Saved'}</p>", errors: literal },
    { code: '<p>{`Saved ${name}`}</p>', errors: literal },
    { code: "<p>{busy ? 'Saving' : t('save')}</p>", errors: literal },
    { code: "<p>{busy && 'Saving'}</p>", errors: literal },
    { code: "<p>{name ?? 'Unknown'}</p>", errors: literal },
    { code: '<Button title="Save" />', errors: literal },
    { code: '<input placeholder="Host" />', errors: literal },
    { code: '<input aria-label="Host" />', errors: literal },
    {
      code: "<input aria-label={busy ? 'Busy' : t('host')} />",
      errors: literal,
    },
    { code: "const item = { label: 'Save' };", errors: literal },
    { code: 'const item = { title: `Save ${name}` };', errors: literal },
    { code: "const item = { description: 'A' + name };", errors: literal },
    { code: '<p>Saved</p>', options: [{ allow: ['Other'] }], errors: literal },
  ],
});
