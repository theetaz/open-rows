// SPDX-License-Identifier: Apache-2.0
import type { SVGProps } from 'react';

const paths = {
  search: 'M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13Zm4.6-1.9L21 21',
  plus: 'M12 5v14M5 12h14',
  refresh: 'M20 8a8 8 0 1 0 0 8M20 3v5h-5',
  sidebar: 'M4 4h16v16H4ZM9 4v16',
  inspector: 'M4 4h16v16H4ZM15 4v16',
  settings: 'M3 7h9m6 0h3M3 17h3m6 0h9M15 4v6M9 14v6',
  database:
    'M5 6c0-1.7 3.1-3 7-3s7 1.3 7 3-3.1 3-7 3-7-1.3-7-3ZM5 6v12c0 1.7 3.1 3 7 3s7-1.3 7-3V6M5 12c0 1.7 3.1 3 7 3s7-1.3 7-3',
};
/** Original line geometry drawn for the gallery, with inherited colour. */
export function Icon({
  name,
  ...props
}: SVGProps<SVGSVGElement> & { name: keyof typeof paths }): React.JSX.Element {
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
      <path d={paths[name]} />
    </svg>
  );
}
