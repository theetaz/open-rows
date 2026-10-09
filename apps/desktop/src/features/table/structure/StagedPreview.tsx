// SPDX-License-Identifier: Apache-2.0
import { DdlPreview } from './forms/DdlPreview';
import { useAlterPreview } from './forms/usePreview';
import type { Structure } from './useStructure';

/** The live DDL of every staged structure change, as the commit would run it. */
export function StagedPreview({
  structure,
}: {
  structure: Structure;
}): React.JSX.Element {
  const changes = structure.schema.history.current.map((item) => item.change);
  const preview = useAlterPreview(structure.target, changes);
  return <DdlPreview state={preview} idle="No structure changes are staged." />;
}
