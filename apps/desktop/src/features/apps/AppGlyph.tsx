// SPDX-License-Identifier: Apache-2.0
import type { AgentApp } from '@openrows/ipc';
import styles from './Apps.module.css';

/** Original marks drawn for OpenRows; they are not the vendors' logos. */
const MARKS: Record<AgentApp, React.JSX.Element> = {
  claude: (
    <path
      d="M10 3.5v13M4.4 6.75l11.2 6.5M4.4 13.25l11.2-6.5"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
    />
  ),
  codex: (
    <>
      <rect
        x="2.75"
        y="3.75"
        width="14.5"
        height="12.5"
        rx="3.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
      />
      <path
        d="M6.5 8l2.5 2-2.5 2M10.75 12.25h3"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </>
  ),
  gemini: (
    <>
      <path
        d="M8 2.5c.5 3 2 4.5 5 5-3 .5-4.5 2-5 5-.5-3-2-4.5-5-5 3-.5 4.5-2 5-5z"
        fill="currentColor"
      />
      <path
        d="M14.5 11.5c.25 1.5 1 2.25 2.5 2.5-1.5.25-2.25 1-2.5 2.5-.25-1.5-1-2.25-2.5-2.5 1.5-.25 2.25-1 2.5-2.5z"
        fill="currentColor"
      />
    </>
  ),
};

/** A tinted tile with the app's glyph; decorative, the name is always beside it. */
export function AppGlyph(props: {
  app: AgentApp;
  size?: 'regular' | 'small';
}): React.JSX.Element {
  return (
    <span
      className={styles.glyph}
      data-app={props.app}
      data-size={props.size ?? 'regular'}
      aria-hidden="true"
    >
      <svg viewBox="0 0 20 20">{MARKS[props.app]}</svg>
    </span>
  );
}
