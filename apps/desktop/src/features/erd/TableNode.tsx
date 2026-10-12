// SPDX-License-Identifier: Apache-2.0
import type { DiagramColumnDto } from '@openrows/ipc';
import { HEADER_HEIGHT, MAX_COLUMNS, PADDING_X, ROW_HEIGHT } from './geometry';
import type { DiagramNode } from './geometry';

/** Average glyph width of 12 px UI text, for clipping long names. */
const CHAR_WIDTH = 6.6;
const MARK_WIDTH = 20;
const MAX_TYPE = 22;

/** `text` cut to `limit` characters with an ellipsis. */
function clip(text: string, limit: number): string {
  if (text.length <= limit) return text;
  return `${text.slice(0, Math.max(limit - 1, 1))}…`;
}

function ColumnRow(props: {
  column: DiagramColumnDto;
  index: number;
  width: number;
  foreign: boolean;
}): React.JSX.Element {
  const { column, width } = props;
  const y = HEADER_HEIGHT + props.index * ROW_HEIGHT + ROW_HEIGHT / 2;
  const type = clip(column.dataType, MAX_TYPE);
  const room = Math.floor((width - 2 * PADDING_X - MARK_WIDTH) / CHAR_WIDTH);
  const mark = column.primaryKey ? 'PK' : props.foreign ? 'FK' : '';
  return (
    <g>
      {mark && (
        <text
          className={column.primaryKey ? 'erd-key' : 'erd-ref'}
          x={PADDING_X}
          y={y}
          dominantBaseline="central"
        >
          {mark}
        </text>
      )}
      <text x={PADDING_X + MARK_WIDTH} y={y} dominantBaseline="central">
        {clip(column.name, Math.max(room - type.length - 1, 4))}
      </text>
      <text
        className="erd-type"
        x={width - PADDING_X}
        y={y}
        textAnchor="end"
        dominantBaseline="central"
      >
        {type}
      </text>
    </g>
  );
}

/**
 * One table box: name header, then columns with PK/FK marks and types.
 * Activating it (click, Return or Space) opens the table.
 */
export function TableNode(props: {
  node: DiagramNode;
  foreign: ReadonlySet<string>;
  match: boolean;
  onOpen: (name: string) => void;
}): React.JSX.Element {
  const { node } = props;
  const { table, width, height } = node;
  const shown = table.columns.slice(0, MAX_COLUMNS);
  return (
    <g
      className="erd-table"
      data-table={table.name}
      data-match={props.match ? '' : undefined}
      transform={`translate(${String(node.x)} ${String(node.y)})`}
      role="button"
      tabIndex={0}
      aria-label={`Open table ${table.name}`}
      onClick={() => {
        props.onOpen(table.name);
      }}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' && event.key !== ' ') return;
        event.preventDefault();
        event.stopPropagation();
        props.onOpen(table.name);
      }}
    >
      <rect className="erd-box" width={width} height={height} rx={6} />
      <path
        className="erd-head"
        d={`M0 6a6 6 0 0 1 6-6h${String(width - 12)}a6 6 0 0 1 6 6v${String(HEADER_HEIGHT - 6)}H0Z`}
      />
      <line
        className="erd-rule"
        x1={0}
        x2={width}
        y1={HEADER_HEIGHT}
        y2={HEADER_HEIGHT}
      />
      <text
        className="erd-title"
        x={PADDING_X}
        y={HEADER_HEIGHT / 2}
        dominantBaseline="central"
      >
        {clip(table.name, Math.floor((width - 2 * PADDING_X) / 7))}
      </text>
      {shown.map((column, index) => (
        <ColumnRow
          key={column.name}
          column={column}
          index={index}
          width={width}
          foreign={props.foreign.has(column.name)}
        />
      ))}
      {node.hidden > 0 && (
        <text
          className="erd-more"
          x={PADDING_X + MARK_WIDTH}
          y={HEADER_HEIGHT + shown.length * ROW_HEIGHT + ROW_HEIGHT / 2}
          dominantBaseline="central"
        >
          {`+${String(node.hidden)} more`}
        </text>
      )}
    </g>
  );
}
