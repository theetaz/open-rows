// SPDX-License-Identifier: Apache-2.0
import type { SVGProps } from 'react';
import { iconPaths } from './paths';
import type { IconName } from './paths';

export type { IconName };

/** Original line geometry drawn for the gallery, with inherited colour. */
export function Icon({
  name,
  ...props
}: SVGProps<SVGSVGElement> & { name: IconName }): React.JSX.Element {
  return (
    <svg
      {...props}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={iconPaths[name]} />
    </svg>
  );
}
