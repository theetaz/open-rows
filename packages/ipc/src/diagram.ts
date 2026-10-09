// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  DiagramFileDto,
  DiagramTarget,
  SchemaDiagramDto,
} from './generated/bindings';
import { unwrap } from './connections';

/** Tables, columns and foreign keys of a schema for the ER diagram (FR-SCHEMA-06). */
export function schemaDiagram(
  target: DiagramTarget,
): Promise<SchemaDiagramDto> {
  return unwrap(commands.schemaDiagram(target));
}

/**
 * Ask where to save an exported diagram in the native save panel, then write
 * it; resolves the written path, or `null` when the panel was cancelled.
 */
export function saveDiagram(
  name: string,
  file: DiagramFileDto,
): Promise<string | null> {
  return unwrap(commands.diagramSave(name, file));
}
