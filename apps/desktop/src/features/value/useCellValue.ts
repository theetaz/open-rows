// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { resultCellInfo, resultCellValue } from '@openrows/ipc';
import type { CellInfoDto } from '@openrows/ipc';
import { AUTO_LOAD_BYTES } from './valueFormat';

/** The result cell an inspector shows: a result id and a row and column index. */
export type CellAddress = { resultId: string; row: number; column: number };

export type CellValueState = {
  /** Kind and size of the cell, once known. */
  info: CellInfoDto | null;
  /** The exact value (text as UTF-8, binary as is), once loaded. */
  bytes: Uint8Array | null;
  /** The value is wanted but has not arrived yet. */
  loading: boolean;
  problem: string | null;
  /** Load the exact value now ("Load full value"). */
  load: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Whether the value of this cell has bytes worth loading. */
const hasBytes = (info: CellInfoDto): boolean =>
  info.kind === 'text' || info.kind === 'binary';

/** Exact byte length of a cell; the wire type allows null, which means none. */
export const byteLenOf = (info: CellInfoDto): number => info.byteLen ?? 0;

/**
 * Kind and size of one cell, then its exact value: loaded at once below
 * `AUTO_LOAD_BYTES`, otherwise only when `load` is called. The caller keys the
 * inspector by cell, so an address never changes under a mounted hook.
 */
export function useCellValue(address: CellAddress): CellValueState {
  const { resultId, row, column } = address;
  const [info, setInfo] = useState<CellInfoDto | null>(null);
  const [bytes, setBytes] = useState<Uint8Array | null>(null);
  const [requested, setRequested] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    resultCellInfo({ resultId, row, column }).then(
      (found) => {
        if (live) setInfo(found);
      },
      (error: unknown) => {
        if (live) setProblem(message(error));
      },
    );
    return () => {
      live = false;
    };
  }, [resultId, row, column]);
  const small =
    info !== null && hasBytes(info) && byteLenOf(info) <= AUTO_LOAD_BYTES;
  const wanted = requested || small;
  useEffect(() => {
    if (!wanted) return undefined;
    let live = true;
    resultCellValue({ resultId, row, column }).then(
      (value) => {
        if (live) setBytes(new Uint8Array(value));
      },
      (error: unknown) => {
        if (live) setProblem(message(error));
      },
    );
    return () => {
      live = false;
    };
  }, [wanted, resultId, row, column]);
  return {
    info,
    bytes,
    loading: wanted && bytes === null && problem === null,
    problem,
    load: () => {
      setRequested(true);
    },
  };
}
