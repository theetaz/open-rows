// SPDX-License-Identifier: Apache-2.0
/** Original 16 px line glyphs of the connection flow (v4 prototype set). */
const glyphs = {
  link: (
    <path d="M6.8 9.2a2.6 2.6 0 0 0 3.7 0l2.2-2.2a2.6 2.6 0 0 0-3.7-3.7l-.7.7M9.2 6.8a2.6 2.6 0 0 0-3.7 0L3.3 9a2.6 2.6 0 0 0 3.7 3.7l.7-.7" />
  ),
  server: (
    <>
      <rect x="2.5" y="2.8" width="11" height="4.4" rx="1.4" />
      <rect x="2.5" y="8.8" width="11" height="4.4" rx="1.4" />
      <path d="M5 5h.01M5 11h.01" />
    </>
  ),
  user: (
    <>
      <circle cx="8" cy="5.5" r="2.7" />
      <path d="M2.8 13.5c.6-2.6 2.6-3.8 5.2-3.8s4.6 1.2 5.2 3.8" />
    </>
  ),
  tag: (
    <path d="M2.5 3.5a1 1 0 0 1 1-1h4.1c.3 0 .5.1.7.3l5 5a1 1 0 0 1 0 1.4l-4.1 4.1a1 1 0 0 1-1.4 0l-5-5a1 1 0 0 1-.3-.7ZM5.3 5.3h.01" />
  ),
  file: (
    <path d="M4.5 2.5h4.6L12 5.4v7.1a1 1 0 0 1-1 1H4.5a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1ZM9 2.5v3h3" />
  ),
  shield: <path d="M8 1.8 13 3.6v4.1c0 3-2 5.3-5 6.5-3-1.2-5-3.5-5-6.5V3.6Z" />,
  route: (
    <>
      <circle cx="3.8" cy="12" r="1.6" />
      <circle cx="12.2" cy="4" r="1.6" />
      <path d="M5.4 12h3.3a2 2 0 0 0 0-4H7.3a2 2 0 0 1 0-4h3.3" />
    </>
  ),
  globe: (
    <>
      <circle cx="8" cy="8" r="5.8" />
      <path d="M2.2 8h11.6M8 2.2c1.8 1.6 2.6 3.5 2.6 5.8S9.8 12.2 8 13.8C6.2 12.2 5.4 10.3 5.4 8S6.2 3.8 8 2.2Z" />
    </>
  ),
  cloud: (
    <path d="M4.6 12.5a2.9 2.9 0 0 1-.4-5.8 3.9 3.9 0 0 1 7.5.9 2.5 2.5 0 0 1-.3 4.9Z" />
  ),
  lock: (
    <>
      <rect x="3.5" y="7" width="9" height="6.5" rx="1.8" />
      <path d="M5.5 7V5.2a2.5 2.5 0 0 1 5 0V7" />
    </>
  ),
  terminal: (
    <>
      <rect x="2" y="3" width="12" height="10" rx="2.2" />
      <path d="m5 6.5 2 1.5-2 1.5M8.5 10H11" />
    </>
  ),
  plug: <path d="M6 2v3.5M10 2v3.5M4 5.5h8v2.2a4 4 0 0 1-8 0ZM8 11.7V14" />,
  bolt: <path d="M8.8 1.8 3.5 9h4l-.8 5.2L12.5 7h-4Z" />,
  history: (
    <path d="M2.8 8a5.2 5.2 0 1 0 1.6-3.7M2.5 2.8v2.6h2.6M8 5.2V8l2 1.3" />
  ),
  settings: (
    <>
      <path d="M2.5 5h6M11.5 5h2M2.5 11h2M7.5 11h6" />
      <circle cx="10" cy="5" r="1.5" />
      <circle cx="6" cy="11" r="1.5" />
    </>
  ),
  sparkles: (
    <path d="M7 2.5 8.2 6 11.5 7.2 8.2 8.4 7 11.8 5.8 8.4 2.5 7.2 5.8 6ZM12 10.5l.6 1.4 1.4.6-1.4.6-.6 1.4-.6-1.4-1.4-.6 1.4-.6Z" />
  ),
  check: <path d="m3.5 8.5 3 3 6-7" />,
  checkCircle: (
    <>
      <circle cx="8" cy="8" r="5.8" />
      <path d="m5.3 8.2 1.9 1.9 3.5-3.9" />
    </>
  ),
  xCircle: (
    <>
      <circle cx="8" cy="8" r="5.8" />
      <path d="m6 6 4 4m0-4-4 4" />
    </>
  ),
  warning: <path d="M8 2.5 14 13H2ZM8 6.5v3M8 11.2h.01" />,
  clock: (
    <>
      <circle cx="8" cy="8" r="5.8" />
      <path d="M8 4.8V8l2.2 1.4" />
    </>
  ),
  spinner: <path d="M8 2.2a5.8 5.8 0 1 0 5.8 5.8" />,
  database: (
    <>
      <ellipse cx="8" cy="4.2" rx="5" ry="2" />
      <path d="M3 4.2v7.6c0 1.1 2.2 2 5 2s5-.9 5-2V4.2M3 8c0 1.1 2.2 2 5 2s5-.9 5-2" />
    </>
  ),
  key: (
    <>
      <circle cx="5.3" cy="10.7" r="2.6" />
      <path d="m7.2 8.8 5.6-5.6M10.6 5.4l1.8 1.8" />
    </>
  ),
} as const;

export type FlowIconName = keyof typeof glyphs;

/** A decorative flow glyph in the inherited colour. */
export function FlowIcon({
  name,
  className,
}: {
  name: FlowIconName;
  className?: string | undefined;
}): React.JSX.Element {
  return (
    <svg
      className={className}
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {glyphs[name]}
    </svg>
  );
}
