// SPDX-License-Identifier: Apache-2.0
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { geometry } from './geometry';

const read = (path: string): string => readFileSync(path, 'utf8');
const controls = read('packages/ui/src/components/Controls.module.css');
const segmented = read('packages/ui/src/components/Segmented.module.css');
const fields = read('packages/ui/src/components/TextField.module.css');

/** Declarations of the rule whose whole selector list is exactly `selector`. */
function rule(css: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = new RegExp(`(?<!,\\n)^${escaped} \\{([^}]*)\\}`, 'm').exec(css);
  if (!match?.[1]) throw new Error(`Missing rule ${selector}`);
  return match[1];
}

const regular = [
  [segmented, '.segmented'],
  [fields, '.frame'],
  [controls, '.group'],
] as const;
const compact = [
  [segmented, ".segmented[data-size='compact']"],
  [fields, ".frame[data-size='compact']"],
] as const;

describe('control consistency (v4, owner rule 1)', () => {
  it('defines one height per control size; every control is a capsule', () => {
    expect(geometry['control-height']).toBe('28px');
    expect(geometry['control-height-compact']).toBe('22px');
    expect(geometry['glass-height']).toBe('32px');
    expect(geometry['radius-capsule']).toBe('999px');
    expect([geometry['switch-width'], geometry['switch-height']]).toEqual([
      '30px',
      '18px',
    ]);
  });
  it('sizes pane bars, the traffic-light row and the address capsule', () => {
    expect(geometry['bar-height']).toBe('38px');
    expect(geometry['traffic-light-row']).toBe(geometry['toolbar-height']);
    expect(geometry['address-width']).toBe('520px');
  });
  it('buttons are capsules at the regular and compact heights', () => {
    const body = rule(controls, '.button');
    expect(body).toContain('--control-height: var(--or-control-height);');
    expect(body).toContain('block-size: var(--control-height);');
    expect(body).toContain('border-radius: var(--or-radius-capsule);');
    const small = rule(controls, ".button[data-size='sm']");
    expect(small).toContain('block-size: var(--or-control-height-compact);');
    expect(small).toContain('border-radius: var(--or-radius-capsule);');
  });
  it.each(regular)(
    '%#: segmented controls, fields and groups are regular capsules',
    (css, selector) => {
      const body = rule(css, selector);
      expect(body).toMatch(
        /block-size: var\(--(or-control-height|control-height)\);/,
      );
      expect(body).toContain('border-radius: var(--or-radius-capsule);');
    },
  );
  it.each(compact)(
    '%#: compact variants are compact capsules',
    (css, selector) => {
      const body = rule(css, selector);
      expect(body).toContain('block-size: var(--or-control-height-compact);');
      expect(body).toContain('border-radius: var(--or-radius-capsule);');
    },
  );
  it('the segmented thumb is a capsule too', () => {
    expect(rule(segmented, '.segmented::before')).toContain(
      'border-radius: var(--or-radius-capsule);',
    );
  });
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
