// SPDX-License-Identifier: Apache-2.0
import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import { noHardcodedUiText } from './scripts/eslint/no-hardcoded-ui-text.mjs';

export default tseslint.config(
  {
    ignores: [
      '**/generated/**',
      '**/dist/**',
      '**/target/**',
      '**/target-*/**',
      '**/node_modules/**',
      '**/gen/**',
    ],
  },
  eslint.configs.recommended,
  {
    files: ['apps/desktop/e2e/**/*.mjs'],
    languageOptions: {
      globals: {
        process: 'readonly',
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        fetch: 'readonly',
        AggregateError: 'readonly',
      },
    },
  },
  ...tseslint.configs.strictTypeChecked.map((config) => ({
    ...config,
    files: ['**/*.ts', '**/*.tsx'],
  })),
  {
    files: ['**/*.{js,mjs,cjs,ts,tsx}'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: '@tauri-apps/api/core',
              message: 'Use @openrows/ipc for typed commands.',
            },
          ],
        },
      ],
    },
  },
  {
    files: ['**/*.ts', '**/*.tsx'],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: { 'react-hooks': reactHooks },
    rules: {
      ...reactHooks.configs.flat.recommended.rules,
      'max-lines-per-function': [
        'error',
        { max: 80, skipBlankLines: true, skipComments: true },
      ],
      'max-lines': ['error', { max: 600 }],
      complexity: ['error', 15],
      'max-depth': ['error', 4],
      'max-params': ['error', 6],
      'no-console': 'error',
      '@typescript-eslint/explicit-module-boundary-types': 'error',
    },
  },
  {
    // Visible text comes from message catalogs (docs/07 §10). The ignored paths
    // hold the catalogs themselves, design tokens, and code that never ships.
    files: ['apps/desktop/src/**/*.{ts,tsx}', 'packages/ui/src/**/*.{ts,tsx}'],
    ignores: [
      '**/*.test.{ts,tsx}',
      '**/*[mM]essages.ts',
      '**/messages/*.ts',
      '**/test[A-Z]*.{ts,tsx}',
      '**/*Fixture.ts',
      'apps/desktop/src/spike/**',
      'apps/desktop/src/test/**',
      'packages/ui/src/tokens/**',
    ],
    plugins: {
      openrows: { rules: { 'no-hardcoded-ui-text': noHardcodedUiText } },
    },
    rules: { 'openrows/no-hardcoded-ui-text': ['error', { allow: [] }] },
  },
  {
    files: ['packages/ipc/**/*.ts'],
    rules: { 'no-restricted-imports': 'off' },
  },
);
