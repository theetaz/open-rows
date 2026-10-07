// SPDX-License-Identifier: Apache-2.0
import type { GridHandle, GridProps, GridTheme } from '@openrows/grid';
/** Dispose even when the lazy renderer resolves after its view was closed. */
export function mountCanvas(
  element: HTMLElement,
  props: GridProps,
  state: {
    theme: React.RefObject<GridTheme>;
    handle: React.RefObject<GridHandle | undefined>;
    onMounted?: () => void;
  },
): () => void {
  let cancelled = false;
  import('@openrows/grid/canvas').then(
    ({ mountGrid }) => {
      if (cancelled) return;
      props.theme = state.theme.current;
      state.handle.current = mountGrid(element, props);
      state.onMounted?.();
    },
    (failure: unknown) => {
      props.onError(failure);
    },
  );
  return () => {
    cancelled = true;
    state.handle.current?.destroy();
    state.handle.current = undefined;
  };
}
