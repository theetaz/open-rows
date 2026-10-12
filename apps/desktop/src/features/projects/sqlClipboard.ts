// SPDX-License-Identifier: Apache-2.0
import { readSqlFile } from '@openrows/ipc';

/** Copy literal SQL, starting the WebKit clipboard write before file I/O resolves. */
export function copyProjectSql(path: string): Promise<void> {
  const sql = readSqlFile(path).then((file) => file.sql);
  if (typeof ClipboardItem === 'undefined')
    return sql.then((text) => navigator.clipboard.writeText(text));
  // WebKit requires the write in the click; the item may resolve asynchronously.
  // https://webkit.org/blog/10855/async-clipboard-api/
  const body = sql.then((text) => new Blob([text], { type: 'text/plain' }));
  return navigator.clipboard.write([new ClipboardItem({ 'text/plain': body })]);
}
