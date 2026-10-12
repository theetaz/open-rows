// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { DiagramTableCheck, useDiagramSelection } from './diagramSelection';
import type { Catalog } from './useCatalog';

function Check(props: { database: string; schema: string }): React.JSX.Element {
  const catalog = { profileId: 'p', ...props } as unknown as Catalog;
  useDiagramSelection(catalog);
  return (
    <DiagramTableCheck
      catalog={catalog}
      object={{
        kind: 'table',
        name: 'same',
        schema: props.schema,
        arguments: null,
        comment: null,
        rowEstimate: null,
      }}
    />
  );
}

it('keeps exact database/schema identities separate even when names contain newlines', () => {
  render(
    <>
      <Check database={'a\nb'} schema="c" />
      <Check database="a" schema={'b\nc'} />
    </>,
  );
  const [first, second] = screen.getAllByRole('checkbox');
  if (!first || !second) throw new Error('Missing selection fixtures');
  fireEvent.click(first);
  expect(first).toBeChecked();
  expect(second).not.toBeChecked();
});
