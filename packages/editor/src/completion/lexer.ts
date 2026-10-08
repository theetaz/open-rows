// SPDX-License-Identifier: Apache-2.0

/** What a token is; comments and whitespace are dropped. */
export type TokenType = 'word' | 'quoted' | 'string' | 'number' | 'punct';

/** One token of SQL text with its document offsets. */
export type SqlToken = {
  type: TokenType;
  /** Identifier name without quotes; the raw text for other tokens. */
  value: string;
  from: number;
  to: number;
};

/**
 * Sticky patterns tried in order at each offset. Unterminated strings,
 * quoted identifiers and comments run to the end of the text, so a token the
 * user is still typing never leaks into the following ones.
 */
const RULES: readonly [TokenType | null, RegExp][] = [
  [null, /\s+|--[^\n]*|\/\*[\s\S]*?(?:\*\/|$)/y],
  ['string', /[EeBbXxNn]?'(?:[^']|'')*'?/y],
  ['string', /\$([A-Za-z_]\w*)?\$[\s\S]*?(?:\$\1\$|$)/y],
  ['quoted', /"(?:[^"]|"")*"?|`(?:[^`]|``)*`?/y],
  ['number', /\d+(?:\.\d*)?(?:[eE][+-]?\d+)?/y],
  ['word', /[A-Za-z_\u0080-￿][\w$\u0080-￿]*/y],
  ['punct', /::|<>|<=|>=|!=|\|\||[^\s]/y],
];

function unquote(text: string): string {
  const quote = text.charAt(0);
  const closed = text.length > 1 && text.endsWith(quote);
  const body = text.slice(1, closed ? -1 : undefined);
  return body.replaceAll(quote + quote, quote);
}

function valueOf(type: TokenType, text: string): string {
  return type === 'quoted' ? unquote(text) : text;
}

/** Tokens of `text`, offsets shifted by `offset` (the text's document start). */
export function tokenize(text: string, offset = 0): SqlToken[] {
  const tokens: SqlToken[] = [];
  let at = 0;
  while (at < text.length) {
    for (const [type, pattern] of RULES) {
      pattern.lastIndex = at;
      const match = pattern.exec(text);
      if (!match?.[0]) continue;
      const end = at + match[0].length;
      if (type)
        tokens.push({
          type,
          value: valueOf(type, match[0]),
          from: offset + at,
          to: offset + end,
        });
      at = end;
      break;
    }
  }
  return tokens;
}

/** True for an identifier-like token: a bare word or a quoted name. */
export function isName(token: SqlToken | undefined): token is SqlToken {
  return token?.type === 'word' || token?.type === 'quoted';
}

/** True when `token` is the bare keyword `word` (case-insensitive). */
export function isWord(token: SqlToken | undefined, word: string): boolean {
  return token?.type === 'word' && token.value.toLowerCase() === word;
}
