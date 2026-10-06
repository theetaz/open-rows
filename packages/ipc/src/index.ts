// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type { AppInfo } from './generated/bindings';

/** Application metadata returned by the Rust shell. */
export type { AppInfo };

/** Read application metadata through the generated command binding. */
export function getAppInfo(): Promise<AppInfo> {
  return commands.appInfo();
}
