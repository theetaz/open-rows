// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';

/** Rows per block: the unit that mounts and unmounts as it scrolls by. */
const BLOCK = 40;
/** Estimated row height until a block has been measured (`--or-tree-row-height`). */
const ROW = 26;
/** Blocks this far outside the scrolled area stay mounted. */
const MARGIN = '600px 0px';

/**
 * Windowed rows: `items` in blocks of 40, each mounted only while it is near
 * the scrolled area and a same-height spacer otherwise. A block that holds a
 * `pinned` item (the selection, an expanded relation) always stays mounted,
 * so focus and open columns survive scrolling. Thousands of tables cost a
 * few dozen rows of DOM.
 */
export function RowBlocks<T>(props: {
  items: readonly T[];
  keyOf: (item: T) => string;
  pinned: (item: T) => boolean;
  children: (item: T) => ReactNode;
}): React.JSX.Element {
  const { items } = props;
  const blocks: T[][] = [];
  for (let start = 0; start < items.length; start += BLOCK)
    blocks.push(items.slice(start, start + BLOCK));
  return (
    <>
      {blocks.map((block, index) => (
        <Block
          key={block[0] ? props.keyOf(block[0]) : index}
          rows={block.length}
          eager={index < 2}
          pinned={block.some(props.pinned)}
        >
          {block.map(props.children)}
        </Block>
      ))}
    </>
  );
}

/** Whether the block is near the scrolled area; always true without an observer. */
function useNear(
  ref: React.RefObject<HTMLDivElement | null>,
  eager: boolean,
): { near: boolean; height: number | null } {
  const observable = typeof IntersectionObserver !== 'undefined';
  const [state, setState] = useState({
    near: eager || !observable,
    height: null as number | null,
  });
  useEffect(() => {
    const element = ref.current;
    if (!element || !observable) return undefined;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry) return;
        const near = entry.isIntersecting;
        setState((previous) =>
          previous.near === near
            ? previous
            : { near, height: entry.boundingClientRect.height },
        );
      },
      { rootMargin: MARGIN },
    );
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [ref, observable]);
  return state;
}

function Block(props: {
  rows: number;
  eager: boolean;
  pinned: boolean;
  children: ReactNode;
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const { near, height } = useNear(ref, props.eager);
  const mounted = near || props.pinned;
  return (
    <div
      ref={ref}
      role="none"
      style={mounted ? undefined : { blockSize: height ?? props.rows * ROW }}
    >
      {mounted && props.children}
    </div>
  );
}
