// SPDX-License-Identifier: Apache-2.0

const PRIMITIVE_LOOKUP = {
  kind: 'function',
  positionalParams: [],
  restParam: 'read',
  calleeEffect: 'read',
  returnType: { kind: 'type', name: 'Primitive' },
  returnValueKind: 'primitive',
};

const MESSAGE_CATALOG = /[mM]essages(\/\w+)?$/;

/**
 * Tells the React Compiler that `t` and `tn` of a message catalog return a
 * string. It otherwise gives every call its own memo slot, which added about
 * 60 bytes per string to the bundle and to each component's cache.
 */
export function messageModuleTypes(moduleName: string): object | null {
  if (!MESSAGE_CATALOG.test(moduleName)) return null;
  return {
    kind: 'object',
    properties: { t: PRIMITIVE_LOOKUP, tn: PRIMITIVE_LOOKUP },
  };
}
