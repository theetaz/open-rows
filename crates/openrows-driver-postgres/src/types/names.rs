// SPDX-License-Identifier: Apache-2.0
//! Built-in type names, array element types and range OIDs from `pg_type.dat`.

/// Array type OID and element OID for every element type the decoder
/// supports natively. Array names come from the session's lookup, which
/// prints them like `integer[]`.
const ARRAYS: [(u32, u32); 25] = [
    (199, 114),
    (651, 650),
    (775, 774),
    (1000, 16),
    (1001, 17),
    (1003, 19),
    (1005, 21),
    (1007, 23),
    (1009, 25),
    (1014, 1042),
    (1015, 1043),
    (1016, 20),
    (1021, 700),
    (1022, 701),
    (1040, 829),
    (1041, 869),
    (1115, 1114),
    (1182, 1082),
    (1183, 1083),
    (1185, 1184),
    (1187, 1186),
    (1231, 1700),
    (1270, 1266),
    (2951, 2950),
    (3807, 3802),
];

/// Built-in range and multirange types (`int4range` … `datemultirange`).
const RANGES: [(u32, &str); 12] = [
    (3904, "int4range"),
    (3906, "numrange"),
    (3908, "tsrange"),
    (3910, "tstzrange"),
    (3912, "daterange"),
    (3926, "int8range"),
    (4451, "int4multirange"),
    (4532, "nummultirange"),
    (4533, "tsmultirange"),
    (4534, "tstzmultirange"),
    (4535, "datemultirange"),
    (4536, "int8multirange"),
];

/// The element type of a natively decoded array type.
pub(crate) fn array_element(type_oid: u32) -> Option<u32> {
    ARRAYS
        .iter()
        .find(|(oid, _)| *oid == type_oid)
        .map(|&(_, element)| element)
}

pub(super) fn is_range(type_oid: u32) -> bool {
    RANGES.iter().any(|(oid, _)| *oid == type_oid)
}

/// Built-in type names; other names come from the session's `pg_type` lookup.
pub(super) fn builtin_name(oid: u32) -> Option<&'static str> {
    scalar_name(oid).or_else(|| RANGES.iter().find(|(range, _)| *range == oid).map(|t| t.1))
}

fn scalar_name(oid: u32) -> Option<&'static str> {
    Some(match oid {
        16 => "bool",
        17 => "bytea",
        19 => "name",
        20 => "int8",
        21 => "int2",
        23 => "int4",
        25 => "text",
        114 => "json",
        650 => "cidr",
        700 => "float4",
        701 => "float8",
        774 => "macaddr8",
        829 => "macaddr",
        869 => "inet",
        1042 => "bpchar",
        1043 => "varchar",
        1082 => "date",
        1083 => "time",
        1114 => "timestamp",
        1184 => "timestamptz",
        1186 => "interval",
        1266 => "timetz",
        1700 => "numeric",
        2950 => "uuid",
        3802 => "jsonb",
        _ => return None,
    })
}
