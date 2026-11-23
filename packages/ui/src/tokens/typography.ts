// SPDX-License-Identifier: Apache-2.0
/**
 * Native system chrome: the system font (SF Pro) at 13 px / 1.385 with
 * −0.006 em tracking, and independently overridable content font roles: data,
 * SQL and identifiers in 12 px ui-monospace (SF Mono) with tabular numbers.
 * Scale 10 · 11 · 12 · 13 · 17 · 22; weights 400 body, 500 controls and tabs,
 * 550–600 current item and headings, 650 titles (DESIGN.md §4).
 */
export const typography = {
  'font-ui':
    '-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", system-ui, "Helvetica Neue", "Segoe UI", sans-serif',
  'font-mono':
    'ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, monospace',
  'font-editor':
    'ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, monospace',
  'font-grid':
    'ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, monospace',
  'font-sidebar':
    '-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", system-ui, "Segoe UI", sans-serif',
  'text-editor': '0.75rem',
  'text-grid': '0.75rem',
  'text-sidebar': '0.8125rem',
  'editor-line-height': '1.5',
  'line-height-ui': '1.385',
  'letter-spacing-ui': '-0.006em',
  'font-smoothing': 'antialiased',
  'font-numeric': 'tabular-nums',
  'text-10': '0.625rem',
  'text-11': '0.6875rem',
  'text-11-5': '0.71875rem',
  'text-12': '0.75rem',
  'text-12-5': '0.78125rem',
  'text-13': '0.8125rem',
  'text-mono': '0.75rem',
  'text-15': '0.9375rem',
  'text-17': '1.0625rem',
  'text-22': '1.375rem',
  'text-30': '1.875rem',
  'weight-regular': '400',
  'weight-medium': '500',
  /** The current item: the active tab, the selected path segment. */
  'weight-current': '550',
  'weight-semibold': '600',
  'weight-title': '650',
  'weight-bold': '700',
};
