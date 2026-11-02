// SPDX-License-Identifier: Apache-2.0
/** Keep unrelated development, provider and benchmark modes out of native tests. */
export function nativeEnvironment(root) {
  const environment = Object.fromEntries(
    Object.entries(process.env).filter(([key]) => !key.startsWith('OPENROWS_')),
  );
  return { ...environment, OPENROWS_E2E_ROOT: root };
}
