// SPDX-License-Identifier: Apache-2.0
import { invoke } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type { CellInfoDto, CellRequest } from './generated/bindings';
import { unwrap } from './connections';
import { rawBytes } from './binary';

/** Kind and exact byte length of one result cell; its value is not copied. */
export function resultCellInfo(request: CellRequest): Promise<CellInfoDto> {
  return unwrap(commands.resultCellInfo(request));
}

/** The exact value of a text (UTF-8) or binary result cell, loaded on demand. */
export async function resultCellValue(
  request: CellRequest,
): Promise<ArrayBuffer> {
  return rawBytes(await invoke<unknown>('result_cell_value', { request }));
}
