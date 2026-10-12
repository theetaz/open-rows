// SPDX-License-Identifier: Apache-2.0
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { DiagramContent } from './DiagramContent';
import { exportDiagram } from './exportImage';
import type { ExportFormat } from './exportImage';
import type { DiagramLayout } from './geometry';

/** Materialize SVG only for an export, including every offscreen canvas table. */
export async function exportLayout(
  format: ExportFormat,
  name: string,
  layout: DiagramLayout,
): Promise<string | null> {
  const host = document.createElement('div');
  host.hidden = true;
  document.body.append(host);
  const root = createRoot(host);
  try {
    flushSync(() => {
      root.render(
        <svg>
          <DiagramContent
            layout={layout}
            matches={new Set()}
            onOpen={() => undefined}
            contentRef={null}
          />
        </svg>,
      );
    });
    const content = host.querySelector<SVGGElement>('[data-diagram]');
    if (!content) throw new Error('Cannot prepare diagram export');
    return await exportDiagram(format, name, { content, size: layout });
  } finally {
    root.unmount();
    host.remove();
  }
}
