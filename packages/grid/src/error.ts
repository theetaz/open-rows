// SPDX-License-Identifier: Apache-2.0
/** Adapter failures identify the boundary that rejected invalid data or unsupported rendering. */
export class GridError extends Error {
  constructor(
    readonly code: 'layout' | 'tile' | 'canvas' | 'adapter',
    message: string,
  ) {
    super(message);
    this.name = 'GridError';
  }
}
