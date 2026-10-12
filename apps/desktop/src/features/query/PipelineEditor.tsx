// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { usePipelineEditor } from './usePipelineEditor';
type Draft = ReturnType<typeof usePipelineEditor>;

/** Stage previews use the same safety admission, cancellation and result pane as MQL. */
export function PipelineEditor({
  run,
  disabled,
}: {
  run: (source: string) => void;
  disabled: boolean;
}): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const draft = usePipelineEditor(run);
  return (
    <section aria-label="Aggregation pipeline">
      <Button
        size="sm"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        Pipeline editor
      </Button>
      {open && (
        <>
          <PipelineFields draft={draft} />
          <StagePreviews draft={draft} disabled={disabled} />
          {draft.error && <p role="alert">{draft.error}</p>}
        </>
      )}
    </section>
  );
}
function PipelineFields({ draft }: { draft: Draft }): React.JSX.Element {
  return (
    <>
      <TextField
        label="Collection"
        value={draft.collection}
        onChange={(event) => {
          draft.setCollection(event.target.value);
        }}
      />
      <label>
        Pipeline JSON
        <textarea
          aria-label="Pipeline JSON"
          rows={6}
          value={draft.text}
          onChange={(event) => {
            draft.setText(event.target.value);
          }}
        />
      </label>
    </>
  );
}
function StagePreviews({
  draft,
  disabled,
}: {
  draft: Draft;
  disabled: boolean;
}): React.JSX.Element {
  return (
    <div role="toolbar" aria-label="Stage previews">
      {Array.from({ length: draft.stages }, (_, index) => (
        <Button
          size="sm"
          key={index}
          disabled={disabled}
          onClick={() => {
            draft.execute(index);
          }}
        >
          Preview stage {String(index + 1)}
        </Button>
      ))}
      <Button
        disabled={disabled}
        onClick={() => {
          draft.execute();
        }}
      >
        Run pipeline
      </Button>
    </div>
  );
}
