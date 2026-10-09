// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from 'react';
import { SegmentedControl } from '@openrows/ui';
import { JsonTree } from './JsonTree';
import { jsonContainer } from './valueKinds';
import { TEXT_SHOWN_CHARS, clipText, formatBytes } from './valueFormat';
import styles from './Value.module.css';

/** Text bytes: a JSON tree when the text is a JSON object or array, else the text itself. */
export function TextValue({ bytes }: { bytes: Uint8Array }): React.JSX.Element {
  const text = useMemo(() => new TextDecoder().decode(bytes), [bytes]);
  const tree = useMemo(() => jsonContainer(text), [text]);
  if (!tree) return <PlainText text={text} byteLen={bytes.length} />;
  return <JsonOrText text={text} tree={tree} byteLen={bytes.length} />;
}

function JsonOrText(props: {
  text: string;
  tree: object;
  byteLen: number;
}): React.JSX.Element {
  const [view, setView] = useState<'tree' | 'text'>('tree');
  return (
    <>
      <SegmentedControl
        aria-label="JSON view"
        options={[
          { value: 'tree', label: 'Tree' },
          { value: 'text', label: 'Text' },
        ]}
        value={view}
        onValueChange={(value) => {
          setView(value === 'text' ? 'text' : 'tree');
        }}
      />
      {view === 'tree' ? (
        <JsonTree value={props.tree} />
      ) : (
        <PlainText text={props.text} byteLen={props.byteLen} />
      )}
    </>
  );
}

function PlainText(props: {
  text: string;
  byteLen: number;
}): React.JSX.Element {
  const shown = clipText(props.text, TEXT_SHOWN_CHARS);
  return (
    <>
      <pre className={styles.text}>{shown.text}</pre>
      {shown.clipped && (
        <p className={styles.note}>
          Showing the first {TEXT_SHOWN_CHARS.toLocaleString('en-US')}{' '}
          characters of {formatBytes(props.byteLen)}.
        </p>
      )}
    </>
  );
}
