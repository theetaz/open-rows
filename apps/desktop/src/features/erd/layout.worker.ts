// SPDX-License-Identifier: Apache-2.0
import type { SchemaDiagramDto } from '@openrows/ipc';
import { layoutDiagram } from './layout';
self.onmessage = (event: MessageEvent<SchemaDiagramDto>): void => {
  try {
    self.postMessage({ layout: layoutDiagram(event.data) });
  } catch (error) {
    self.postMessage({
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
