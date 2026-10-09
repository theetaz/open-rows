// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { samplePlan } from '../../test/planStub';
import { ExplainView } from './ExplainView';

function renderView(plan = samplePlan()): ReturnType<typeof userEvent.setup> {
  const user = userEvent.setup();
  render(<ExplainView plan={plan} />);
  return user;
}

const details = (): HTMLElement =>
  screen.getByRole('complementary', { name: 'Operator details' });

it('shows operators as a tree coloured by heat with hotspot badges', () => {
  renderView();
  const tree = screen.getByRole('tree', { name: 'Plan operators' });
  const items = within(tree).getAllByRole('treeitem');
  expect(items).toHaveLength(4);
  expect(items[2]).toHaveAttribute('aria-level', '3');
  expect(items[2]).toHaveAttribute('data-heat', 'high');
  expect(items[3]).toHaveAttribute('data-heat', 'medium');
  expect(
    within(items[2] as HTMLElement).getByText('Rows ×200'),
  ).toBeInTheDocument();
  expect(
    within(items[0] as HTMLElement).getByText('Disk spill'),
  ).toBeInTheDocument();
  expect(screen.getByText(/Execution 7\.80 ms/)).toHaveTextContent(
    'Changes rolled back',
  );
});

it('selects operators by click, hotspot chip and arrow keys', async () => {
  const user = renderView();
  expect(
    within(details()).getByRole('heading', { level: 3 }),
  ).toHaveTextContent('Sort');
  await user.click(screen.getByRole('treeitem', { name: /Index Scan/ }));
  expect(
    within(details()).getByRole('heading', { level: 3 }),
  ).toHaveTextContent('Index Scan on public.orders using orders_customer');
  expect(within(details()).getByText('Shared hit')).toBeInTheDocument();
  const hotspots = screen.getByRole('list', { name: 'Hotspots' });
  await user.click(
    within(hotspots).getByRole('button', { name: /55% of plan/ }),
  );
  expect(
    within(details()).getByRole('heading', { level: 3 }),
  ).toHaveTextContent('Seq Scan');
  expect(within(details()).getByText("(region = 'eu')")).toBeInTheDocument();
  screen.getByRole('tree').focus();
  await user.keyboard('{ArrowUp}');
  expect(
    within(details()).getByRole('heading', { level: 3 }),
  ).toHaveTextContent('Nested Loop');
  await user.keyboard('{ArrowLeft}');
  expect(screen.getAllByRole('treeitem')).toHaveLength(2);
  await user.keyboard('{ArrowRight}');
  expect(screen.getAllByRole('treeitem')).toHaveLength(4);
});

it('collapses an operator with its twisty', async () => {
  const user = renderView();
  await user.click(
    screen.getAllByRole('button', { name: 'Collapse' })[1] as HTMLElement,
  );
  expect(screen.getAllByRole('treeitem')).toHaveLength(2);
  expect(screen.getByRole('button', { name: 'Expand' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
});

it('shows and copies the raw plan', async () => {
  const user = renderView();
  const writeText = vi
    .spyOn(navigator.clipboard, 'writeText')
    .mockResolvedValue();
  await user.click(screen.getByRole('button', { name: 'Copy Plan' }));
  expect(writeText).toHaveBeenCalledWith('[{"Plan": {"Node Type": "Sort"}}]');
  expect(
    await screen.findByRole('button', { name: 'Copied' }),
  ).toBeInTheDocument();
  await user.click(screen.getByRole('radio', { name: 'Raw' }));
  expect(screen.getByLabelText('Raw plan')).toHaveTextContent(
    '"Node Type": "Sort"',
  );
  expect(screen.queryByRole('tree')).not.toBeInTheDocument();
});

it('reports a plan without hotspots or weights plainly', () => {
  const plan = samplePlan({ weight: 'none', analyzed: false });
  plan.nodes = plan.nodes.map((node) => ({ ...node, hotspots: [], share: 0 }));
  renderView(plan);
  expect(screen.getByText('No hotspots found.')).toBeInTheDocument();
  expect(
    within(details()).queryByText('Share of plan'),
  ).not.toBeInTheDocument();
});
