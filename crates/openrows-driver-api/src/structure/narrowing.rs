// SPDX-License-Identifier: Apache-2.0
//! Whether a column type change can lose or reject existing values: a
//! shorter length, lower precision or scale, a smaller integer or float, or
//! a conversion to another type family (anything but text that holds every
//! value). A heuristic for the review sheet's warnings, shared by every
//! engine's generator.

/// What a type text says about the values it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    /// Integers of this many bytes.
    Integer(u8),
    /// Floating point of this many bytes.
    Float(u8),
    /// Exact numerics: precision and scale (`None` is unconstrained).
    Decimal(Option<u32>, Option<u32>),
    /// Text of at most this many characters (`None` is unlimited).
    Text(Option<u64>),
    /// Any other type, compared by its normalised name.
    Other,
}

/// The review warning for changing `column` from `old` to `new`, if the
/// change can truncate, round or reject values.
#[must_use]
pub fn narrowing_warning(column: &str, old: &str, new: &str) -> Option<String> {
    narrows(old, new).then(|| {
        format!(
            "Changing {column:?} from {} to {} can truncate, round or reject existing values.",
            normalise(old),
            normalise(new)
        )
    })
}

/// True when values of `old` may not fit `new`.
#[must_use]
pub fn narrows(old: &str, new: &str) -> bool {
    let (old_name, new_name) = (normalise(old), normalise(new));
    if old_name == new_name {
        return false;
    }
    match (shape(&old_name), shape(&new_name)) {
        (Shape::Integer(from), Shape::Integer(to)) | (Shape::Float(from), Shape::Float(to)) => {
            to < from
        }
        (Shape::Decimal(from, from_scale), Shape::Decimal(to, to_scale)) => {
            shrinks(from_scale, to_scale) || shrinks(from, to)
        }
        (Shape::Text(from), Shape::Text(to)) => shrinks(from, to),
        (Shape::Integer(_), Shape::Decimal(None, _) | Shape::Float(8)) | (_, Shape::Text(None)) => {
            false
        }
        _ => true,
    }
}

/// A limit (`None` is unlimited) gets smaller.
fn shrinks<T: Ord>(from: Option<T>, to: Option<T>) -> bool {
    match (from, to) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(from), Some(to)) => to < from,
    }
}

/// Lower case, single spaces, no space before `(`.
fn normalise(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
        .replace(" (", "(")
        .replace(", ", ",")
}

/// The base name and the integer modifiers of a normalised type.
fn parts(name: &str) -> (&str, Vec<u64>) {
    let Some((base, rest)) = name.split_once('(') else {
        return (name, Vec::new());
    };
    let inside = rest.split(')').next().unwrap_or_default();
    let numbers = inside
        .split(',')
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    (base.trim(), numbers)
}

fn shape(name: &str) -> Shape {
    let (base, numbers) = parts(name);
    let base = base.trim_end_matches(" unsigned");
    let first = numbers.first().copied();
    match base {
        "tinyint" => Shape::Integer(1),
        "smallint" | "int2" | "smallserial" => Shape::Integer(2),
        "mediumint" => Shape::Integer(3),
        "int" | "integer" | "int4" | "serial" => Shape::Integer(4),
        "bigint" | "int8" | "bigserial" => Shape::Integer(8),
        "real" | "float4" => Shape::Float(4),
        "float" if first.is_some_and(|bits| bits <= 24) => Shape::Float(4),
        "double precision" | "float8" | "double" | "float" => Shape::Float(8),
        "numeric" | "decimal" | "dec" => decimal(&numbers),
        "char" | "character" | "nchar" | "bpchar" => Shape::Text(Some(first.unwrap_or(1))),
        "varchar" | "character varying" | "nvarchar" | "varchar2" => Shape::Text(first),
        "tinytext" => Shape::Text(Some(255)),
        "mediumtext" | "longtext" | "clob" | "citext" | "string" | "text" => Shape::Text(None),
        _ => Shape::Other,
    }
}

fn decimal(numbers: &[u64]) -> Shape {
    let precision = numbers.first().and_then(|value| u32::try_from(*value).ok());
    let scale = numbers
        .get(1)
        .and_then(|value| u32::try_from(*value).ok())
        .or(precision.map(|_| 0));
    Shape::Decimal(precision, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorter_or_smaller_types_narrow() {
        for (old, new) in [
            ("varchar(100)", "varchar(50)"),
            ("character varying(40)", "char(10)"),
            ("text", "varchar(255)"),
            ("bigint", "integer"),
            ("int", "smallint"),
            ("numeric(10,2)", "numeric(8,2)"),
            ("numeric(10, 4)", "numeric(10,2)"),
            ("numeric", "numeric(12,2)"),
            ("double precision", "real"),
            ("text", "integer"),
            ("timestamp", "date"),
            ("integer", "varchar(5)"),
        ] {
            assert!(narrows(old, new), "{old} -> {new}");
        }
    }

    #[test]
    fn widening_and_equal_types_do_not() {
        for (old, new) in [
            ("varchar(50)", "varchar(100)"),
            ("VARCHAR(50)", "varchar (50)"),
            ("integer", "bigint"),
            ("smallint", "int4"),
            ("numeric(8,2)", "numeric(10,2)"),
            ("numeric(8,2)", "numeric"),
            ("integer", "text"),
            ("timestamp", "text"),
            ("char(1)", "varchar(10)"),
            ("varchar(20)", "longtext"),
            ("real", "double precision"),
            ("int", "numeric"),
        ] {
            assert!(!narrows(old, new), "{old} -> {new}");
        }
    }

    #[test]
    fn warnings_name_the_column_and_both_types() {
        let warning = narrowing_warning("price", "numeric(10, 2)", "INTEGER");
        assert_eq!(
            warning.as_deref(),
            Some(
                "Changing \"price\" from numeric(10,2) to integer can truncate, round or reject existing values."
            )
        );
        assert_eq!(narrowing_warning("id", "integer", "bigint"), None);
    }
}
