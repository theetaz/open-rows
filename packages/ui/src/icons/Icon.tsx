// SPDX-License-Identifier: Apache-2.0
import type { SVGProps } from 'react';
import { iconPaths } from './paths';
import type { IconName } from './paths';

export type { IconName };

/**
 * Original line geometry in a 16 px box with a 1.5 px stroke at any size, in the
 * inherited colour: components choose it (tertiary in lists and trees,
 * secondary on icon buttons, the label colour in toolbar capsules).
 */
export function Icon({
  name,
  ...props
}: SVGProps<SVGSVGElement> & { name: IconName }): React.JSX.Element {
  return (
    <svg
      width="16"
      height="16"
      {...props}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={iconPaths[name]} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}
