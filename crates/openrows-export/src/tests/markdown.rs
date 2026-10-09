// SPDX-License-Identifier: Apache-2.0
use super::{TestResult, export_one, mixed, texts};
use crate::Format;

#[test]
fn a_table_with_escaped_cells_and_right_aligned_numbers() -> TestResult {
    let text = export_one(&Format::Markdown, &mixed()?.slice(0, 1))?;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        "| id | big | ok | ratio | price | name | doc | raw | uid | day | at |"
    );
    assert_eq!(
        lines[1],
        "| ---: | ---: | --- | ---: | ---: | --- | --- | --- | --- | --- | --- |"
    );
    assert!(lines[2].starts_with("| -7 | 18446744073709551615 | true | 0.5 | -12.34 |"));
    let batch = texts(&[Some("a|b\\c\r\nd\ne"), None, Some("")])?;
    assert_eq!(
        export_one(&Format::Markdown, &batch)?,
        "| v |\n| --- |\n| a\\|b\\\\c<br>d<br>e |\n| NULL |\n|  |\n"
    );
    Ok(())
}
