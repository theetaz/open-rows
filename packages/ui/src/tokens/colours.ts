// SPDX-License-Identifier: Apache-2.0
/**
 * Light palette: the approved "quiet native" kit (design/revamp v3, kit.css).
 * Older token names keep their meaning and take the kit's value, e.g.
 * `control-fill` is the kit's `--fill`, `placeholder` its `--null`, `row-stripe`
 * its `--zebra`, `cell-edited` its `--edit-soft`; docs/20 §2 has the full map.
 * Status words (`*-fg`) and dark `secondary` are the kit's hues nudged by at
 * most 11 % so a pill or label keeps 4.5:1 on every surface it is drawn on.
 * Glass is only for floating things: the sidebar panel, toolbar capsules,
 * menus, popovers and sheets. Content surfaces are solid.
 */
export const light = {
  /** Solid window content and the grid (`--bg`). */
  page: '#FFFFFF',
  window: '#FFFFFF',
  /** Grouped inset sections and the pending bar (`--group`). */
  group: '#F5F5F7',
  /** A field or well set on a group (`--raised`). */
  raised: '#FFFFFF',
  panel: 'rgba(245,245,247,0.95)',
  'panel-solid': '#F5F5F7',
  toolbar: '#FFFFFF',
  /** The window under the floating sidebar, over the native material. */
  'window-veil': 'rgba(243,244,246,0.68)',
  'glass-sidebar': 'rgba(255,255,255,0.5)',
  'glass-sidebar-edge':
    'inset 0 0 0 1px rgba(255,255,255,0.5), inset 0 1px 0 rgba(255,255,255,0.85), 0 0 0 1px rgba(0,0,0,0.05), 0 6px 20px rgba(0,0,0,0.06)',
  /** Toolbar capsules (`--glass-control`). */
  'glass-fill': 'rgba(255,255,255,0.8)',
  'glass-fill-solid': '#FFFFFF',
  'glass-control-edge':
    'inset 0 0 0 1px transparent, 0 0 0 1px rgba(0,0,0,0.06), 0 1px 2px rgba(0,0,0,0.05), 0 3px 8px rgba(0,0,0,0.04)',
  /** Menus, popovers, the palette. */
  'glass-float': 'rgba(250,250,252,0.78)',
  /** Sheets and alerts: nearly solid so forms stay legible. */
  'glass-sheet': 'rgba(255,255,255,0.98)',
  /** Address capsule in the toolbar and its text. */
  address: 'rgba(118,118,128,0.12)',
  'address-text': '#1D1D1F',
  'label-tertiary': '#636366',
  'field-box-fill': '#FFFFFF',
  'field-box-stroke': 'rgba(60,60,67,0.24)',
  'picker-fill': 'rgba(118,118,128,0.12)',
  'pane-separator': 'rgba(60,60,67,0.13)',
  'glyph-table': '#8E8E93',
  'glyph-view': '#8E8E93',
  /** Every second grid row (`--zebra`). */
  'row-stripe': 'rgba(118,118,128,0.05)',
  'row-inserted': 'rgba(42,122,69,0.11)',
  'row-deleted': 'rgba(179,54,46,0.09)',
  /** A staged cell (`--edit-soft`). */
  'cell-edited': 'rgba(176,124,30,0.14)',
  /** Primary label (`--label`). */
  text: '#1D1D1F',
  'syntax-keyword': '#7A3E9D',
  'syntax-string': '#9B4A2F',
  'syntax-number': '#2C64C6',
  'syntax-function': '#2F6F73',
  'syntax-comment': '#72727A',
  secondary: '#636366',
  /** Icons and marks only, never text. */
  tertiary: '#8E8E93',
  /** NULL, EMPTY, DEFAULT markers and placeholders (`--null`). */
  placeholder: '#72727A',
  separator: 'rgba(60,60,67,0.13)',
  'separator-strong': 'rgba(60,60,67,0.24)',
  /** Column rules in the grid. */
  'grid-line': 'rgba(60,60,67,0.07)',
  /** Neutral chart marks (bars, secondary series); one accent series at most. */
  'chart-mark': 'rgba(142,142,147,0.6)',
  /** Control fills (`--fill`) and pressed fills or tracks (`--fill-2`). */
  'control-fill': 'rgba(118,118,128,0.12)',
  'fill-2': 'rgba(118,118,128,0.2)',
  hover: 'rgba(0,0,0,0.04)',
  press: 'rgba(0,0,0,0.08)',
  'control-hover': 'rgba(118,118,128,0.2)',
  /** The selected segment (`--segment-on`). */
  'control-active': '#FFFFFF',
  'segment-shadow': '0 0 0 1px rgba(0,0,0,0.04), 0 1px 2px rgba(0,0,0,0.1)',
  'control-stroke': 'rgba(60,60,67,0.24)',
  'switch-track': 'rgba(118,118,128,0.2)',
  'switch-knob': '#FFFFFF',
  'shadow-knob': '0 0 0 0.5px rgba(0,0,0,0.08), 0 1px 3px rgba(0,0,0,0.2)',
  'sheet-scrim': 'rgba(0,0,0,0.16)',
  'shadow-sheet': '0 0 0 1px rgba(0,0,0,0.1), 0 24px 60px rgba(0,0,0,0.2)',
  /** The accent as text or an icon. */
  accent: '#2C64C6',
  /** The accent as a fill under white text. */
  'accent-fill': '#2C64C6',
  /** Selection tint. */
  'accent-soft': 'rgba(44,100,198,0.13)',
  'on-accent': '#FFFFFF',
  'connection-pg': '#1160AA',
  'connection-prod': '#C8445A',
  'connection-ch': '#805E0A',
  'connection-sqlite': '#536778',
  destructive: '#B3362E',
  /** Solid focus outline (≥ 3:1); text fields use the soft `field-focus` ring. */
  focus: '#2C64C6',
  'field-focus': 'rgba(44,100,198,0.45)',
  'success-bg': 'rgba(42,122,69,0.11)',
  'success-fg': '#287442',
  'warning-bg': 'rgba(138,90,18,0.11)',
  'warning-fg': '#8A5A12',
  'danger-bg': 'rgba(179,54,46,0.09)',
  'danger-fg': '#B3362E',
  'info-bg': 'rgba(44,100,198,0.13)',
  'info-fg': '#2A60BE',
  'neutral-bg': 'rgba(118,118,128,0.12)',
  'neutral-fg': '#636366',
  'local-bg': 'rgba(42,122,69,0.11)',
  'local-fg': '#287442',
  'dev-bg': 'rgba(42,122,69,0.11)',
  'dev-fg': '#287442',
  'staging-bg': 'rgba(138,90,18,0.11)',
  'staging-fg': '#8A5A12',
  'prod-bg': 'rgba(179,54,46,0.09)',
  'prod-fg': '#B3362E',
  /** Environment dots: a dot and a word, never a tinted window. */
  'tag-local': '#2A7A45',
  'tag-dev': '#2A7A45',
  'tag-staging': '#8A5A12',
  'tag-prod': '#B3362E',
  /** Menus, popovers and tooltips (`--glass-float-edge`). */
  'panel-shadow':
    'inset 0 0 0 1px rgba(255,255,255,0.6), 0 0 0 1px rgba(0,0,0,0.08), 0 8px 24px rgba(0,0,0,0.1)',
};
/** Dark palette from the same kit; tinted fills are composited in contrast tests. */
export const dark: typeof light = {
  page: '#1C1C1E',
  window: '#1C1C1E',
  group: '#28282B',
  raised: '#37373B',
  panel: 'rgba(40,40,43,0.95)',
  'panel-solid': '#28282B',
  toolbar: '#1C1C1E',
  'window-veil': 'rgba(30,30,32,0.68)',
  'glass-sidebar': 'rgba(72,72,78,0.3)',
  'glass-sidebar-edge':
    'inset 0 0 0 1px rgba(255,255,255,0.08), inset 0 1px 0 rgba(255,255,255,0.12), 0 0 0 1px rgba(0,0,0,0.28), 0 6px 20px rgba(0,0,0,0.24)',
  'glass-fill': 'rgba(255,255,255,0.07)',
  'glass-fill-solid': '#2C2C2F',
  'glass-control-edge':
    'inset 0 0 0 1px rgba(255,255,255,0.08), 0 0 0 1px transparent, 0 1px 2px rgba(0,0,0,0.25), 0 3px 8px rgba(0,0,0,0.18)',
  'glass-float': 'rgba(44,44,48,0.8)',
  'glass-sheet': 'rgba(36,36,39,0.98)',
  address: 'rgba(118,118,128,0.24)',
  'address-text': '#F5F5F7',
  'label-tertiary': '#A6A6AA',
  'field-box-fill': '#37373B',
  'field-box-stroke': 'rgba(255,255,255,0.2)',
  'picker-fill': 'rgba(118,118,128,0.24)',
  'pane-separator': 'rgba(255,255,255,0.09)',
  'glyph-table': '#6E6E73',
  'glyph-view': '#6E6E73',
  'row-stripe': 'rgba(255,255,255,0.028)',
  'row-inserted': 'rgba(108,192,138,0.16)',
  'row-deleted': 'rgba(232,131,123,0.16)',
  'cell-edited': 'rgba(217,169,85,0.17)',
  text: '#F5F5F7',
  'syntax-keyword': '#C59BDC',
  'syntax-string': '#E0957F',
  'syntax-number': '#6EA6F2',
  'syntax-function': '#79C0C4',
  'syntax-comment': '#8A8A92',
  secondary: '#A6A6AA',
  tertiary: '#6E6E73',
  placeholder: '#8A8A92',
  separator: 'rgba(255,255,255,0.09)',
  'separator-strong': 'rgba(255,255,255,0.2)',
  'grid-line': 'rgba(255,255,255,0.055)',
  'chart-mark': 'rgba(142,142,147,0.55)',
  'control-fill': 'rgba(118,118,128,0.24)',
  'fill-2': 'rgba(118,118,128,0.36)',
  hover: 'rgba(255,255,255,0.05)',
  press: 'rgba(255,255,255,0.1)',
  'control-hover': 'rgba(118,118,128,0.36)',
  'control-active': 'rgba(255,255,255,0.18)',
  'segment-shadow': '0 0 0 1px rgba(0,0,0,0.1), 0 1px 2px rgba(0,0,0,0.3)',
  'control-stroke': 'rgba(255,255,255,0.2)',
  'switch-track': 'rgba(118,118,128,0.36)',
  'switch-knob': '#FFFFFF',
  'shadow-knob': '0 0 0 0.5px rgba(0,0,0,0.2), 0 1px 3px rgba(0,0,0,0.4)',
  'sheet-scrim': 'rgba(0,0,0,0.42)',
  'shadow-sheet':
    '0 0 0 1px rgba(255,255,255,0.1), 0 24px 60px rgba(0,0,0,0.55)',
  accent: '#6EA6F2',
  'accent-fill': '#2F6BD0',
  'accent-soft': 'rgba(110,166,242,0.2)',
  'on-accent': '#FFFFFF',
  'connection-pg': '#1160AA',
  'connection-prod': '#C8445A',
  'connection-ch': '#805E0A',
  'connection-sqlite': '#536778',
  destructive: '#E8837B',
  focus: '#6EA6F2',
  'field-focus': 'rgba(110,166,242,0.5)',
  'success-bg': 'rgba(108,192,138,0.16)',
  'success-fg': '#6CC08A',
  'warning-bg': 'rgba(217,169,85,0.16)',
  'warning-fg': '#D9A955',
  'danger-bg': 'rgba(232,131,123,0.16)',
  'danger-fg': '#EA8D86',
  'info-bg': 'rgba(110,166,242,0.2)',
  'info-fg': '#7EB0F3',
  'neutral-bg': 'rgba(118,118,128,0.24)',
  'neutral-fg': '#A6A6AA',
  'local-bg': 'rgba(108,192,138,0.16)',
  'local-fg': '#6CC08A',
  'dev-bg': 'rgba(108,192,138,0.16)',
  'dev-fg': '#6CC08A',
  'staging-bg': 'rgba(217,169,85,0.16)',
  'staging-fg': '#D9A955',
  'prod-bg': 'rgba(232,131,123,0.16)',
  'prod-fg': '#EA8D86',
  'tag-local': '#6CC08A',
  'tag-dev': '#6CC08A',
  'tag-staging': '#D9A955',
  'tag-prod': '#E8837B',
  'panel-shadow':
    'inset 0 0 0 1px rgba(255,255,255,0.1), 0 0 0 1px rgba(0,0,0,0.4), 0 8px 24px rgba(0,0,0,0.4)',
};
/** Stronger separators and opaque materials under increased contrast. */
export const highContrast = {
  light: {
    separator: '#606060',
    'separator-strong': '#606060',
    'grid-line': '#8A8A8A',
    panel: light['panel-solid'],
    'control-fill': '#E4E4E7',
    'control-stroke': '#606060',
    'pane-separator': '#606060',
    'switch-track': '#8A8A8A',
    'field-focus': light.focus,
  },
  dark: {
    separator: '#C0C0C0',
    'separator-strong': '#C0C0C0',
    'grid-line': '#7A7A7A',
    panel: dark['panel-solid'],
    'control-fill': '#3A3A3E',
    'control-stroke': '#C0C0C0',
    'pane-separator': '#C0C0C0',
    'switch-track': '#7A7A7A',
    'field-focus': dark.focus,
  },
};
