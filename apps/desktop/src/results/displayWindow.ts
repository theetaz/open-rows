// SPDX-License-Identifier: Apache-2.0
import { resultWindow } from '@openrows/ipc';
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import { ArrowWorker } from './workerClient';

let shared: ArrowWorker | undefined;
/** One Arrow decode worker for every result grid, started on first use. */
function worker(): ArrowWorker {
  shared ??= new ArrowWorker();
  return shared;
}

/** Fetch a result window and convert it to display cells off the main thread. */
export async function displayWindow(
  request: WindowRequest,
): Promise<DisplayTile> {
  return worker().display(await resultWindow(request), request);
}
