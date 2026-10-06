// SPDX-License-Identifier: Apache-2.0
import styles from './HighlightText.module.css';
/** Highlight matched character positions without modifying the accessible text. */
export function HighlightText({
  text,
  indices,
}: {
  text: string;
  indices: readonly number[];
}): React.JSX.Element {
  const matched = new Set(indices);
  let offset = 0;
  return (
    <span>
      {Array.from(text).map((character) => {
        const index = offset;
        offset += character.length;
        return matched.has(index) ? (
          <mark className={styles.match} key={index}>
            {character}
          </mark>
        ) : (
          character
        );
      })}
    </span>
  );
}
