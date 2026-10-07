// SPDX-License-Identifier: Apache-2.0
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { geometry } from './geometry';

const read = (path: string): string => readFileSync(path, 'utf8');
const controls = read('packages/ui/src/components/Controls.module.css');
const fields = read('packages/ui/src/components/TextField.module.css');

/** Declarations of the rule whose whole selector list is exactly `selector`. */
function rule(css: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = new RegExp(`(?<!,\\n)^${escaped} \\{([^}]*)\\}`, 'm').exec(css);
  if (!match?.[1]) throw new Error(`Missing rule ${selector}`);
  return match[1];
}

const regular = [
  [controls, '.button'],
  [controls, '.segmented'],
  [fields, '.frame'],
] as const;
const compact = [
  [controls, ".button[data-size='sm']"],
  [controls, ".segmented[data-size='compact']"],
  [fields, ".frame[data-size='compact']"],
] as const;

describe('control consistency (docs/20 §3.3)', () => {
  it('defines one height and one radius per control size', () => {
    expect(geometry['control-height']).toBe('28px');
    expect(geometry['control-height-compact']).toBe('24px');
    expect(geometry['radius-control']).toBe('7px');
    expect(geometry['radius-control-compact']).toBe('6px');
    expect([geometry['switch-width'], geometry['switch-height']]).toEqual([
      '26px',
      '15px',
    ]);
  });
  it.each(regular)(
    '%#: buttons, segmented controls and fields share the regular tokens',
    (css, selector) => {
      const body = rule(css, selector);
      expect(body).toContain('block-size: var(--or-control-height);');
      expect(body).toContain('border-radius: var(--or-radius-control);');
    },
  );
  it.each(compact)(
    '%#: compact variants share the compact tokens',
    (css, selector) => {
      const body = rule(css, selector);
      expect(body).toContain('block-size: var(--or-control-height-compact);');
      expect(body).toContain(
        'border-radius: var(--or-radius-control-compact);',
      );
    },
  );
  it('selects and search fields reuse the text-field frame', () => {
    const select = read('packages/ui/src/components/SelectField.tsx');
    expect(select).toContain("from './TextField.module.css'");
    expect(select).toContain('className={styles.frame}');
    expect(rule(fields, '.frame input,\n.frame select')).toContain(
      'block-size: 100%;',
    );
  });
  it('switches are macOS-sized', () => {
    const body = rule(controls, '.switch');
    expect(body).toContain('inline-size: var(--or-switch-width);');
    expect(body).toContain('block-size: var(--or-switch-height);');
  });
});

function stylesheets(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return stylesheets(path);
    return entry.name.endsWith('.css') ? [path] : [];
  });
}

it('native controls keep the arrow cursor (no pointer hand)', () => {
  const owned = [
    ...stylesheets('packages/ui/src'),
    'apps/desktop/src/features/connections/ConnectionSheet.module.css',
    'apps/desktop/src/features/connections/ConnectionHeader.module.css',
    'apps/desktop/src/features/table/Table.module.css',
  ];
  for (const path of owned)
    expect(read(path), path).not.toMatch(/cursor:\s*pointer/);
});
