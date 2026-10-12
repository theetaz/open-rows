// SPDX-License-Identifier: Apache-2.0
import type { SchemaDiagramDto } from '@openrows/ipc';
import type { DiagramLayout } from './geometry';

/** Own one layout job; terminate it when completed, failed or cancelled. */
export function startLayout(diagram: SchemaDiagramDto): {
  result: Promise<DiagramLayout>;
  cancel: () => void;
} {
  const worker = new Worker(new URL('./layout.worker.ts', import.meta.url), {
    type: 'module',
  });
  let rejectJob: (error: Error) => void = () => undefined;
  const result = new Promise<DiagramLayout>((resolve, reject) => {
    rejectJob = reject;
    worker.onmessage = (
      event: MessageEvent<{ layout?: DiagramLayout; error?: string }>,
    ) => {
      worker.terminate();
      if (event.data.layout) resolve(event.data.layout);
      else reject(new Error(event.data.error ?? 'Diagram layout failed'));
    };
    worker.onerror = (event) => {
      worker.terminate();
      reject(new Error(event.message || 'Diagram layout worker failed'));
    };
    worker.onmessageerror = () => {
      worker.terminate();
      reject(new Error('Cannot read diagram worker reply'));
    };
    try {
      worker.postMessage(diagram);
    } catch (error) {
      worker.terminate();
      reject(error instanceof Error ? error : new Error(String(error)));
    }
  });
  return {
    result,
    cancel: () => {
      worker.terminate();
      rejectJob(new Error('Diagram layout cancelled'));
    },
  };
}
