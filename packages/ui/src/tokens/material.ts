// SPDX-License-Identifier: Apache-2.0
/** Glass blur recipes for floating things, and functional motion only. */
export const material = {
  /** The floating sidebar panel. */
  'glass-panel': 'blur(30px) saturate(140%)',
  /** Menus, popovers, sheets and the palette. */
  'glass-toolbar': 'blur(24px) saturate(140%)',
  'motion-quick': '100ms',
  'motion-popover': '160ms',
  'motion-panel': '360ms',
  'motion-disclosure': '220ms',
  'motion-easing': 'cubic-bezier(0.23, 1, 0.32, 1)',
};
