// SPDX-License-Identifier: Apache-2.0
/** Reduced transparency: every glass recipe becomes its solid surface. */
export const SOLID: Readonly<Record<string, string>> = {
  panel: 'var(--or-panel-solid)',
  'glass-panel': 'none',
  'glass-toolbar': 'none',
  'blur-float': 'none',
  toolbar: 'var(--or-page)',
  'glass-fill': 'var(--or-glass-fill-solid)',
  'window-tint': 'var(--or-panel-solid)',
  'glass-sidebar': 'var(--or-panel-solid)',
  'window-veil': 'var(--or-page)',
  'glass-float': 'var(--or-glass-fill-solid)',
  'glass-sheet': 'var(--or-glass-fill-solid)',
};
