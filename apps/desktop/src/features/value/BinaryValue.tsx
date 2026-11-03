// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from 'react';
import { SegmentedControl } from '@openrows/ui';
import { hexDump } from './hexDump';
import { imageMime } from './valueKinds';
import type { ImageMime } from './valueKinds';
import { HEX_SHOWN_BYTES, formatBytes } from './valueFormat';
import styles from './Value.module.css';
import { t } from './messages';

/** Binary bytes: an image preview when the magic bytes name an image, and the hex view. */
export function BinaryValue({
  bytes,
}: {
  bytes: Uint8Array;
}): React.JSX.Element {
  const mime = useMemo(() => imageMime(bytes), [bytes]);
  if (!mime) return <HexView bytes={bytes} />;
  return <ImageOrHex bytes={bytes} mime={mime} />;
}

function ImageOrHex(props: {
  bytes: Uint8Array;
  mime: ImageMime;
}): React.JSX.Element {
  const [view, setView] = useState<'image' | 'hex'>('image');
  return (
    <>
      <SegmentedControl
        aria-label={t('binaryView')}
        options={[
          { value: 'image', label: t('image') },
          { value: 'hex', label: t('hex') },
        ]}
        value={view}
        onValueChange={(value) => {
          setView(value === 'hex' ? 'hex' : 'image');
        }}
      />
      {view === 'image' ? (
        <ImagePreview bytes={props.bytes} mime={props.mime} />
      ) : (
        <HexView bytes={props.bytes} />
      )}
    </>
  );
}

/** `offset | hex | ASCII` of the first bytes, with a note when the value is longer. */
function HexView({ bytes }: { bytes: Uint8Array }): React.JSX.Element {
  const dump = useMemo(() => hexDump(bytes, HEX_SHOWN_BYTES), [bytes]);
  return (
    <>
      <pre className={styles.hex} aria-label={t('hexDump')}>
        {dump.text}
      </pre>
      {dump.shown < bytes.length && (
        <p className={styles.note}>
          {t('showingFirstShown', {
            shown: formatBytes(dump.shown),
            count: formatBytes(bytes.length),
          })}
        </p>
      )}
    </>
  );
}

const CHUNK = 0x8000;

/** A `data:` URL of the image: no object URL to revoke, and the image never leaves the page. */
function dataUrl(bytes: Uint8Array, mime: ImageMime): string {
  let binary = '';
  for (let start = 0; start < bytes.length; start += CHUNK)
    binary += String.fromCharCode(...bytes.subarray(start, start + CHUNK));
  return `data:${mime};base64,${btoa(binary)}`;
}

function ImagePreview(props: {
  bytes: Uint8Array;
  mime: ImageMime;
}): React.JSX.Element {
  const source = useMemo(
    () => dataUrl(props.bytes, props.mime),
    [props.bytes, props.mime],
  );
  return (
    <img
      className={styles.image}
      src={source}
      alt={t('imageValueMime', { mime: props.mime })}
    />
  );
}
