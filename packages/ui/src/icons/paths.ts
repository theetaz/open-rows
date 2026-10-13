// SPDX-License-Identifier: Apache-2.0
/** Original 24×24 line geometry, stroked with the current colour. */
export const iconPaths = {
  star: 'M12 3l2.8 5.7 6.2.9-4.5 4.4 1.1 6.2L12 17.3l-5.6 2.9 1.1-6.2L3 9.6l6.2-.9Z',
  search: 'M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13Zm4.6-1.9L21 21',
  plus: 'M12 5v14M5 12h14',
  refresh: 'M20 8a8 8 0 1 0 0 8M20 3v5h-5',
  sidebar: 'M4 4h16v16H4ZM9 4v16',
  inspector: 'M4 4h16v16H4ZM15 4v16',
  settings: 'M3 7h9m6 0h3M3 17h3m6 0h9M15 4v6M9 14v6',
  chevronDown: 'M6 9l6 6 6-6',
  chevronUpDown: 'M8 9l4-4 4 4M8 15l4 4 4-4',
  eye: 'M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12ZM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z',
  eyeOff:
    'M2 12s3.6-7 10-7c2 0 3.8.7 5.2 1.6M22 12s-3.6 7-10 7c-2 0-3.8-.7-5.2-1.6M9.9 9.9a3 3 0 0 0 4.2 4.2M4 4l16 16',
  key: 'M15.5 13a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9ZM12.3 11.7 4 20M6 18l2.5 2.5M8.5 15.5l2 2',
  database:
    'M5 6c0-1.7 3.1-3 7-3s7 1.3 7 3-3.1 3-7 3-7-1.3-7-3ZM5 6v12c0 1.7 3.1 3 7 3s7-1.3 7-3V6M5 12c0 1.7 3.1 3 7 3s7-1.3 7-3',
  chevronLeft: 'M14.5 6l-6 6 6 6',
  chevronRight: 'M9.5 6l6 6-6 6',
  arrowRight: 'M5 12h14M13 6l6 6-6 6',
  copy: 'M9 9h10v11H9ZM15 9V5H5v11h4',
  lock: 'M6 11h12v9H6ZM8.5 11V8a3.5 3.5 0 0 1 7 0v3',
  unlock: 'M6 11h12v9H6ZM8.5 11V8a3.5 3.5 0 0 1 6.9-.8',
  terminal: 'M4 5h16v14H4ZM8 10l2.5 2L8 14M12.5 15h3.5',
  command:
    'M9 9h6v6H9ZM9 9H7a2 2 0 1 1 2-2v2M15 9V7a2 2 0 1 1 2 2h-2M15 15h2a2 2 0 1 1-2 2v-2M9 15v2a2 2 0 1 1-2-2h2',
  filter: 'M4 5h16l-6 7.5V19l-4-2v-4.5Z',
  columns: 'M4 5h16v14H4ZM9.5 5v14M14.5 5v14',
  table: 'M4 5h16v14H4ZM4 10h16M4 14.5h16M10 10v9',
  code: 'M8 8l-4 4 4 4M16 8l4 4-4 4M13.5 5.5l-3 13',
  more: 'M5.5 12h.01M12 12h.01M18.5 12h.01',
  close: 'M7 7l10 10M17 7 7 17',
  pin: 'M9 4h6M10 4v6l-3 4h10l-3-4V4M12 14v6',
  function:
    'M14.5 4.5c-2.2-.6-3.4.6-3.9 3L8.9 16.5c-.5 2.4-1.7 3.6-3.9 3M7.5 10H15',
  list: 'M9 7h11M9 12h11M9 17h11M4.5 7h.01M4.5 12h.01M4.5 17h.01',
  column: 'M4 5h16v14H4ZM9.5 5v14M14.5 5v14M9.5 9.5h5M9.5 14h5',
  schema: 'M3.5 6.5h6l2 2h9v10h-17Z',
  diagram: 'M3 4h7v6H3ZM14 14h7v6h-7ZM6.5 10v7h7.5',
  minus: 'M5 12h14',
  fit: 'M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5',
  jobs: 'M12 4v10M7.5 9.5 12 14l4.5-4.5M4 15v4h16v-4',
  calendar: 'M4 6h16v14H4ZM4 10h16M8 3v5M16 3v5',
  sparkles:
    'M10 4l1.6 4.4L16 10l-4.4 1.6L10 16l-1.6-4.4L4 10l4.4-1.6ZM18 14l.8 2.2L21 17l-2.2.8L18 20l-.8-2.2L15 17l2.2-.8Z',
} as const;

/** Names of the original line icons. */
export type IconName = keyof typeof iconPaths;
