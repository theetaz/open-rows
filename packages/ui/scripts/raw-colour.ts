// SPDX-License-Identifier: Apache-2.0
/** Reject literal colour notation in token-only component stylesheets. */
export function hasRawColour(css: string): boolean {
  return /#(?:[\da-f]{3,8})\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\s*\(/i.test(
    css,
  );
}
