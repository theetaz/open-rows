// SPDX-License-Identifier: Apache-2.0
import { EngineIcon, isEngineId } from '@openrows/ui';
import styles from './EnginePicker.module.css';

/** The engine's original glyph tile (no vendor logos); `id` is an engine choice id. */
export function EngineTile({
  id,
  size = 'large',
}: {
  id: string;
  size?: 'large' | 'small';
}): React.JSX.Element | null {
  if (!isEngineId(id)) return null;
  return (
    <EngineIcon
      engine={id}
      size={size === 'large' ? 'lg' : 'md'}
      className={styles.tile}
    />
  );
}
