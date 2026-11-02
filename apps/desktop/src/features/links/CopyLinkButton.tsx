// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { copyLink } from './linkText';

/**
 * Copy the table's `openrows://` link with its applied filters. Links carry
 * parameterised filters only, so raw SQL filters and OR groups disable it.
 */
export function CopyLinkButton(props: {
  link: string | null;
}): React.JSX.Element {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      size="sm"
      disabled={props.link === null}
      title={
        props.link === null
          ? 'Links carry only filters that must all match, without raw SQL'
          : 'Copy an openrows:// link to this table with these filters'
      }
      onClick={() => {
        if (props.link === null) return;
        copyLink(props.link).then(
          () => {
            setCopied(true);
          },
          () => undefined,
        );
      }}
    >
      {copied ? 'Link Copied' : 'Copy Link'}
    </Button>
  );
}
