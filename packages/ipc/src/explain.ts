// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type { ExplainReply, ExplainRequest } from './generated/bindings';
import { unwrap } from './connections';

/**
 * Explain the statement at the cursor, or the selection, on an open
 * session. `confirmationRequired`, `blocked` and `cancelled` replies mean
 * no plan; `cancelQuery(explainId)` cancels a running explain.
 */
export function explainQuery(request: ExplainRequest): Promise<ExplainReply> {
  return unwrap(commands.queryExplain(request));
}
