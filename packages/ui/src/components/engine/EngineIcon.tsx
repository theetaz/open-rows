// SPDX-License-Identifier: Apache-2.0
import { engineTiles } from '../../tokens/engines';
import { engineGlyphs } from './engineGlyphs';
import type { EngineId, GlyphShape } from './engineGlyphs';
import styles from './EngineIcon.module.css';

export type { EngineId };

function Shape({ shape }: { shape: GlyphShape }): React.JSX.Element {
  if ('d' in shape) return <path d={shape.d} />;
  if ('circle' in shape) {
    const [cx, cy, r] = shape.circle;
    return <circle cx={cx} cy={cy} r={r} />;
  }
  if ('ellipse' in shape) {
    const [cx, cy, rx, ry] = shape.ellipse;
    return <ellipse cx={cx} cy={cy} rx={rx} ry={ry} />;
  }
  const [x, y, width, height, rx] = shape.rect;
  return <rect x={x} y={y} width={width} height={height} rx={rx} />;
}

/** Whether `id` names an engine with a tile (e.g. a stored engine string). */
export function isEngineId(id: string): id is EngineId {
  return id in engineGlyphs;
}

/**
 * An engine tile: an original glyph on the engine colour, 18 (`sm`, lists and
 * path bar), 24 (`md`, rows) or 36 px (`lg`, the engine picker). Decorative:
 * the engine name is always written next to it.
 *
 * ```tsx
 * <EngineIcon engine="postgres" size="sm" />
 * ```
 */
export function EngineIcon({
  engine,
  size = 'md',
  className = '',
}: {
  engine: EngineId;
  size?: 'sm' | 'md' | 'lg';
  className?: string | undefined;
}): React.JSX.Element {
  const colours = engineTiles[engine];
  const style = {
    '--engine-bg': colours.bg,
    '--engine-ink': colours.ink,
  } as React.CSSProperties;
  return (
    <span
      className={`${styles.tile ?? ''} ${className}`}
      data-size={size}
      data-engine={engine}
      style={style}
      aria-hidden="true"
    >
      <svg viewBox="0 0 20 20" focusable="false">
        {engineGlyphs[engine].map((shape, index) => (
          <Shape key={index} shape={shape} />
        ))}
      </svg>
    </span>
  );
}
