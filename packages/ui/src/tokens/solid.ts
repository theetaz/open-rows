// SPDX-License-Identifier: Apache-2.0
/** Reduced transparency: every glass recipe becomes its solid surface. */
export const SOLID: Readonly<Record<string, string>> = {
  panel: 'var(--or-panel-solid)',
  'glass-panel': 'none',
  'glass-toolbar': 'none',
  toolbar: 'var(--or-page)',
  'glass-fill': 'var(--or-glass-fill-solid)',
  'glass-sidebar': 'var(--or-panel-solid)',
  'window-veil': 'var(--or-panel-solid)',
  'glass-float': 'var(--or-panel-solid)',
  'glass-sheet': 'var(--or-page)',
};
