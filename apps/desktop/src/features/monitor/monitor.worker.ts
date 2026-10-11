// SPDX-License-Identifier: Apache-2.0
import { decodeTable } from './monitorArrow';

self.onmessage = ({
  data,
}: MessageEvent<{ id: number; bytes: ArrayBuffer }>) => {
  try {
    self.postMessage({ id: data.id, table: decodeTable(data.bytes) });
  } catch {
    self.postMessage({ id: data.id, error: 'Invalid monitor Arrow page' });
  }
};
