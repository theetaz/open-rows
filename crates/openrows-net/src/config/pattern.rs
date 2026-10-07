// SPDX-License-Identifier: Apache-2.0
//! OpenSSH host pattern matching: `*`, `?` and `!` negation, case-insensitive.

/// Case-insensitive glob match for host names.
pub(crate) fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().flat_map(char::to_lowercase).collect();
    let text: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    glob_chars(&pattern, &text)
}

/// Case-sensitive glob match for file names (`Include` patterns).
pub(crate) fn file_glob_match(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    glob_chars(&pattern, &name)
}

/// Match `text` against a glob with `*` (any run) and `?` (one character).
///
/// Iterative with single-star backtracking: linear memory, no recursion, so
/// hostile patterns cannot overflow the stack.
fn glob_chars(pattern: &[char], text: &[char]) -> bool {
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((star_p, star_t)) => {
                    p = star_p + 1;
                    t = star_t + 1;
                    star = Some((star_p, star_t + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

/// Match a pattern list (`Host a b !c`): any positive match and no negated match.
pub(crate) fn pattern_list_matches<S: AsRef<str>>(patterns: &[S], host: &str) -> bool {
    let mut matched = false;
    for pattern in patterns {
        let pattern = pattern.as_ref();
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard_match(negated, host) {
                return false;
            }
        } else if wildcard_match(pattern, host) {
            matched = true;
        }
    }
    matched
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{pattern_list_matches, wildcard_match};

    #[test]
    fn wildcards_follow_openssh_semantics() {
        assert!(wildcard_match("*.example.com", "db.Example.COM"));
        assert!(wildcard_match("db-?", "db-1"));
        assert!(!wildcard_match("db-?", "db-10"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("a*b*c", "aXXbYYc"));
        assert!(!wildcard_match("a*b*c", "aXXbYY"));
    }

    #[test]
    fn negation_overrides_positive_matches() {
        let patterns = ["*.internal", "!secret.internal"];
        assert!(pattern_list_matches(&patterns, "db.internal"));
        assert!(!pattern_list_matches(&patterns, "secret.internal"));
        assert!(!pattern_list_matches(&["!only"], "other"));
    }

    proptest! {
        #[test]
        fn literal_patterns_match_themselves(text in "[a-z0-9.-]{0,24}") {
            prop_assert!(wildcard_match(&text, &text));
            prop_assert!(wildcard_match("*", &text));
        }

        #[test]
        fn never_panics_on_arbitrary_input(pattern in ".{0,32}", text in ".{0,32}") {
            let _ = wildcard_match(&pattern, &text);
        }
    }
}
