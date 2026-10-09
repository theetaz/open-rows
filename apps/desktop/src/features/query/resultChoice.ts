// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';

/**
 * The chosen result tab: a new extra tab (plan) is shown when it arrives,
 * a new run's results when it starts; otherwise the user's choice stays.
 */
export function useResultChoice(
  extraId: string | null,
  queryId: string | null,
): [string | null, (id: string | null) => void] {
  const [chosen, setChosen] = useState<string | null>(extraId);
  const [seen, setSeen] = useState({ extraId, queryId });
  if (seen.extraId !== extraId || seen.queryId !== queryId) {
    setSeen({ extraId, queryId });
    if (extraId && extraId !== seen.extraId) setChosen(extraId);
    else if (queryId && queryId !== seen.queryId) setChosen(null);
  }
  return [chosen, setChosen];
}
