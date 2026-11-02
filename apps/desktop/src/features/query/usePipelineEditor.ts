// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { pipelineQuery } from './pipeline';

type PipelineDraft = {
  collection: string;
  setCollection: (value: string) => void;
  text: string;
  setText: (value: string) => void;
  error: string | null;
  execute: (through?: number) => void;
  /** The operator of each stage (`$match`), or `stage` when it has none. */
  stages: string[];
};

/** Draft pipeline state with explicit validation before running a stage. */
export function usePipelineEditor(
  run: (source: string) => void,
): PipelineDraft {
  const [collection, setCollection] = useState('');
  const [text, setText] = useState('[\n  {"$match": {}}\n]');
  const [error, setError] = useState<string | null>(null);
  const execute = (through?: number): void => {
    try {
      if (!collection.trim()) throw new Error('Enter a collection name.');
      const source = pipelineQuery(collection, text, through);
      setError(null);
      run(source);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    }
  };
  return {
    collection,
    setCollection,
    text,
    setText,
    error,
    execute,
    stages: stageNames(text),
  };
}

function stageNames(text: string): string[] {
  try {
    const value: unknown = JSON.parse(text);
    if (!Array.isArray(value)) return [];
    return value
      .slice(0, 100)
      .map((stage: unknown) =>
        stage !== null && typeof stage === 'object' && !Array.isArray(stage)
          ? (Object.keys(stage)[0] ?? 'stage')
          : 'stage',
      );
  } catch {
    return [];
  }
}
