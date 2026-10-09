// SPDX-License-Identifier: Apache-2.0
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIST: &str = ";
; Archive created at 2026-10-09 10:00:00 UTC
;     dbname: shop
;
; Selected TOC Entries:
;
5; 2615 2200 SCHEMA - public pg_database_owner
215; 1259 16386 TABLE public accounts postgres
3412; 0 16386 TABLE DATA public accounts postgres
3263; 2606 16391 CONSTRAINT public accounts accounts_pkey postgres
";

#[test]
fn parses_entries_and_skips_comments() {
    let entries = parse_list(LIST);
    let ids: Vec<u32> = entries.iter().map(|entry| entry.id).collect();
    assert_eq!(ids, [5, 215, 3412, 3263]);
    assert_eq!(entries[1].label, "TABLE public accounts postgres");
    assert!(entries[2].data && !entries[1].data);
    assert!(parse_list("garbage\n12;\n").is_empty());
}

#[test]
fn use_lists_keep_archive_order() {
    let entries = parse_list(LIST);
    assert_eq!(
        use_list(&entries, &[3263, 215]),
        "215; 1259 16386 TABLE public accounts postgres\n\
         3263; 2606 16391 CONSTRAINT public accounts accounts_pkey postgres\n"
    );
}

#[test]
fn detects_archives_by_their_first_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let custom = dir.path().join("a.dump");
    std::fs::write(&custom, b"PGDMP\x01\x0e\x00")?;
    assert_eq!(detect(&custom)?, DumpKind::PgCustom);
    let mut tar = vec![0_u8; 512];
    tar[257..262].copy_from_slice(b"ustar");
    let tar_path = dir.path().join("a.tar");
    std::fs::write(&tar_path, tar)?;
    assert_eq!(detect(&tar_path)?, DumpKind::PgTar);
    let sql = dir.path().join("a.sql");
    std::fs::write(&sql, "CREATE TABLE t (id int);")?;
    assert_eq!(detect(&sql)?, DumpKind::PlainSql);
    assert!(!DumpKind::PlainSql.is_archive() && DumpKind::PgTar.is_archive());
    let folder = dir.path().join("backup");
    std::fs::create_dir(&folder)?;
    assert!(detect(&folder).is_err());
    std::fs::write(folder.join("toc.dat"), b"PGDMP")?;
    assert_eq!(detect(&folder)?, DumpKind::PgDirectory);
    assert!(detect(&dir.path().join("missing")).is_err());
    Ok(())
}
