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
  stages: number;
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
    stages: stageCount(text),
  };
}
function stageCount(text: string): number {
  try {
    const value: unknown = JSON.parse(text);
    return Array.isArray(value) ? Math.min(value.length, 100) : 0;
  } catch {
    return 0;
  }
}
