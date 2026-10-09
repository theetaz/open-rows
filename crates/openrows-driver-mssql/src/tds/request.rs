// SPDX-License-Identifier: Apache-2.0
//! Request payloads: a SQL batch (MS-TDS 2.2.6.7) and an `sp_executesql`
//! call (2.2.6.6) whose parameters are `nvarchar(max)` text. Both start
//! with the transaction descriptor header the server requires (2.2.5.3).

use super::collation::Collation;
use super::cursor::utf16_bytes;

/// `sp_executesql` in the RPC procedure-id form.
const SP_EXECUTESQL: u16 = 10;
/// `nvarchar` type code.
const NVARCHAR: u8 = 0xE7;
/// Bytes per `PLP` chunk of a parameter value.
const PLP_CHUNK: usize = 1 << 20;

/// `ALL_HEADERS` with the transaction descriptor (0 outside a transaction).
fn all_headers(transaction: u64, out: &mut Vec<u8>) {
    out.extend_from_slice(&22_u32.to_le_bytes());
    out.extend_from_slice(&18_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&transaction.to_le_bytes());
    out.extend_from_slice(&1_u32.to_le_bytes());
}

/// A SQL batch running `sql`.
pub(crate) fn sql_batch(sql: &str, transaction: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(22 + sql.len() * 2);
    all_headers(transaction, &mut out);
    out.extend(utf16_bytes(sql));
    out
}

/// `sp_executesql @stmt = sql, @params = N'@P1 nvarchar(max), …',
/// @P1 = …`: every value is Unicode text (`None` is NULL) that the server
/// converts to the type each placeholder needs.
pub(crate) fn execute_sql(
    sql: &str,
    params: &[Option<String>],
    (transaction, collation): (u64, Collation),
) -> Vec<u8> {
    let mut out = Vec::new();
    all_headers(transaction, &mut out);
    out.extend_from_slice(&u16::MAX.to_le_bytes());
    out.extend_from_slice(&SP_EXECUTESQL.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    let declarations = (1..=params.len())
        .map(|number| format!("@P{number} nvarchar(max)"))
        .collect::<Vec<_>>()
        .join(", ");
    text_param("@stmt", Some(sql), collation, &mut out);
    text_param("@params", Some(&declarations), collation, &mut out);
    for (index, value) in params.iter().enumerate() {
        let name = format!("@P{}", index + 1);
        text_param(&name, value.as_deref(), collation, &mut out);
    }
    out
}

/// One input parameter of type `nvarchar(max)` with a single-chunk value.
fn text_param(name: &str, value: Option<&str>, collation: Collation, out: &mut Vec<u8>) {
    let name = utf16_bytes(name);
    // Parameter names are at most a few characters long.
    out.push(u8::try_from(name.len() / 2).unwrap_or(u8::MAX));
    out.extend(name);
    out.push(0);
    out.push(NVARCHAR);
    out.extend_from_slice(&u16::MAX.to_le_bytes());
    out.extend_from_slice(&collation.to_bytes());
    let Some(value) = value else {
        out.extend_from_slice(&u64::MAX.to_le_bytes());
        return;
    };
    let bytes = utf16_bytes(value);
    out.extend_from_slice(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    for chunk in bytes.chunks(PLP_CHUNK) {
        // Chunks are at most `PLP_CHUNK` bytes, so the length fits.
        let len = u32::try_from(chunk.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&0_u32.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_carry_the_transaction_and_utf16_text() {
        let batch = sql_batch("SELECT 1", 7);
        assert_eq!(&batch[..4], &22_u32.to_le_bytes());
        assert_eq!(&batch[10..18], &7_u64.to_le_bytes());
        assert_eq!(&batch[22..], &utf16_bytes("SELECT 1")[..]);
    }

    #[test]
    fn executesql_declares_one_nvarchar_per_value() {
        let rpc = execute_sql(
            "SELECT @P1",
            &[Some("x".into()), None],
            (0, Collation::default()),
        );
        let text = String::from_utf8_lossy(&rpc).replace('\0', "");
        assert!(text.contains("@P1 nvarchar(max), @P2 nvarchar(max)"));
        assert_eq!(&rpc[22..26], &[0xFF, 0xFF, 10, 0]);
        assert!(rpc.ends_with(&u64::MAX.to_le_bytes()));
    }
}
