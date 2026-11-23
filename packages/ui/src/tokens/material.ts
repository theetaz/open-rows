// SPDX-License-Identifier: Apache-2.0
/**
 * Blur recipes for floating things, and functional motion only (v4, about a
 * third faster than the prototype). Only transform and opacity animate; colour
 * may cross-fade for `motion-instant` on hover and press.
 */
export const material = {
  /** Floating panels painted over web content (no native material under them). */
  'glass-panel': 'blur(40px) saturate(170%)',
  /** Sheets and the palette. */
  'glass-toolbar': 'blur(50px) saturate(180%)',
  /** Menus, popovers and pickers. */
  'blur-float': 'blur(50px) saturate(180%)',
  /** Hover and press fills. */
  'motion-instant': '60ms',
  /** Menu fade-out, tree chevron, inspector field swap. */
  'motion-quick': '90ms',
  /** Menus open. */
  'motion-menu': '60ms',
  /** Popovers and pickers in, and out. */
  'motion-popover': '110ms',
  'motion-popover-out': '70ms',
  /** Tab reflow, view swap, step change, path segment swap. */
  'motion-standard': '140ms',
  /** A dropped tab settles. */
  'motion-settle': '160ms',
  /** Sidebar and inspector. */
  'motion-panel': '180ms',
  'motion-disclosure': '140ms',
  /** Anything arriving. */
  'motion-easing': 'cubic-bezier(0.22, 1, 0.36, 1)',
  /** Small overshoot: tab drop, switch knob, test tick. */
  'motion-spring': 'cubic-bezier(0.34, 1.42, 0.64, 1)',
  /** Anything leaving. */
  'motion-exit': 'cubic-bezier(0.4, 0, 1, 1)',
};
