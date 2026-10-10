// SPDX-License-Identifier: Apache-2.0
import { useContext } from 'react';
import { SettingsQuery } from './settingsSearch';

/** Highlight literal case-insensitive search words without accepting markup. */
export function HighlightMatch({ text }: { text: string }): React.JSX.Element {
  const words = useContext(SettingsQuery).trim().split(/\s+/).filter(Boolean);
  const escaped = words.map((word) =>
    word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'),
  );
  const parts = escaped.length
    ? text.split(new RegExp(`(${escaped.join('|')})`, 'gi'))
    : [text];
  return (
    <>
      {parts.map((part, index) =>
        words.some((word) => word.toLowerCase() === part.toLowerCase()) ? (
          <mark key={index}>{part}</mark>
        ) : (
          part
        ),
      )}
    </>
  );
}
