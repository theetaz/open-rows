// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { CellPainter, booleanText, isMuted, textWidth } from './cellPaint';
import { gridTheme } from './theme';
import type { DisplayCell, GridColumn } from './model';

/** Records the fill colour of every text and rectangle draw. */
function recorder(): {
  context: CanvasRenderingContext2D;
  texts: { text: string; colour: string; align: string; x: number }[];
  rects: string[];
  stroke: ReturnType<typeof vi.fn>;
} {
  const texts: { text: string; colour: string; align: string; x: number }[] =
      [],
    rects: string[] = [];
  const context = {
    fillStyle: '',
    textAlign: 'left',
    fillText(text: string, x: number) {
      texts.push({ text, colour: this.fillStyle, align: this.textAlign, x });
    },
    fillRect() {
      rects.push(this.fillStyle);
    },
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    stroke: vi.fn(),
  };
  return {
    context: context as unknown as CanvasRenderingContext2D,
    texts,
    rects,
    stroke: context.stroke,
  };
}
const { colours } = gridTheme('light');
const text: GridColumn = { id: 't', title: 't', width: 120, pinned: false };
const flag: GridColumn = { ...text, kind: 'boolean' };
function paint(cell: DisplayCell, column: GridColumn, selected = false) {
  const canvas = recorder(),
    painter = new CellPainter(colours);
  Object.assign(painter.box, { x: 0, y: 0, width: 120, height: 24 });
  painter.selected = selected;
  painter.background(canvas.context, cell);
  painter.content(canvas.context, cell, column, cell.text);
  return canvas;
}

it('paints NULL and EMPTY as distinct muted markers, and real text normally', () => {
  const nullCell = paint({ text: 'NULL', state: 'null' }, text);
  const emptyCell = paint({ text: 'EMPTY', state: 'empty' }, text);
  const value = paint({ text: 'NULL', state: 'normal' }, text);
  expect(nullCell.texts[0]).toMatchObject({
    text: 'NULL',
    colour: colours.tertiary,
  });
  expect(emptyCell.texts[0]).toMatchObject({
    text: 'EMPTY',
    colour: colours.tertiary,
  });
  expect(value.texts[0]?.colour).toBe(colours.text);
});

it('mutes staged NULL, EMPTY and DEFAULT markers and outlines staged cells', () => {
  for (const marker of ['null', 'empty', 'default'] as const)
    expect(isMuted({ text: marker, state: 'changed', marker })).toBe(true);
  expect(isMuted({ text: 'x', state: 'changed' })).toBe(false);
  const staged = paint(
    { text: 'DEFAULT', state: 'changed', marker: 'default' },
    text,
  );
  expect(staged.texts[0]?.colour).toBe(colours.tertiary);
  expect(staged.rects).toContain(colours['cell-edited']);
  expect(
    staged.rects.filter((fill) => fill === colours.destructive),
  ).toHaveLength(8);
  const inserted = paint({ text: 'v', state: 'inserted' }, text);
  expect(inserted.rects).toEqual([colours['row-inserted']]);
});

it('centres booleans as TRUE/FALSE with a stepper; NULL booleans stay NULL', () => {
  expect([booleanText('true'), booleanText('f'), booleanText('x')]).toEqual([
    'TRUE',
    'FALSE',
    'x',
  ]);
  const on = paint({ text: 'true', state: 'normal' }, flag);
  expect(on.texts[0]).toMatchObject({ text: 'TRUE', align: 'center', x: 52 });
  expect(on.stroke).toHaveBeenCalledOnce();
  const missing = paint({ text: 'NULL', state: 'null' }, flag);
  expect(missing.texts[0]?.text).toBe('NULL');
  expect(
    paint({ text: 'a', state: 'normal' }, text).stroke,
  ).not.toHaveBeenCalled();
  expect(
    paint({ text: 'a', state: 'normal' }, { ...text, kind: 'enum' }).stroke,
  ).toHaveBeenCalledOnce();
  expect(textWidth(flag)).toBe(textWidth(text) - 16);
});

it('right-aligns numbers and uses on-accent text in selected rows', () => {
  const number = paint(
    { text: '42', state: 'normal' },
    { ...text, align: 'end' },
  );
  expect(number.texts[0]).toMatchObject({ align: 'right', x: 112 });
  const selected = paint({ text: 'NULL', state: 'null' }, text, true);
  expect(selected.texts[0]?.colour).toBe(colours['on-accent']);
});
