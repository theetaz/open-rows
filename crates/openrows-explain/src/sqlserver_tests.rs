// SPDX-License-Identifier: Apache-2.0
//! Golden native showplan models and defensive streaming limits.
use super::*;
use crate::{Hotspot, analyse};

const SCAN: &str = include_str!("../fixtures/ms_scan.xml");
const SEEK: &str = include_str!("../fixtures/ms_seek.xml");
const JOIN: &str = include_str!("../fixtures/ms_join.xml");
const UPDATE: &str = include_str!("../fixtures/ms_update.xml");

#[test]
fn captured_scan_seek_join_and_update_models() -> Result<(), ParseError> {
    let scan = parse(SCAN)?;
    assert_eq!(scan.root.data.node_type, "Table Scan");
    assert_eq!(scan.root.data.relation.as_deref(), Some("dbo.explain_t"));
    assert!(scan.root.data.full_scan);
    assert_eq!(scan.root.data.estimated_rows, Some(20000.0));
    assert!(!scan.analyzed);
    let seek = parse(SEEK)?;
    assert_eq!(seek.root.data.node_type, "Clustered Index Seek");
    assert_eq!(seek.root.data.relation.as_deref(), Some("dbo.explain_keys"));
    assert!(seek.root.data.index.is_some());
    let join = analyse(parse(JOIN)?);
    assert!(
        join.nodes
            .iter()
            .any(|node| node.data.node_type == "Hash Match")
    );
    assert!(join.nodes.iter().any(|node| {
        node.hotspots
            .iter()
            .any(|hotspot| matches!(hotspot, Hotspot::Spill { .. }))
    }));
    let update = parse(UPDATE)?;
    assert_eq!(update.root.data.node_type, "Table Update");
    assert!(update.analyzed);
    assert_eq!(update.root.data.actual_rows, Some(10.0));
    Ok(())
}

#[test]
fn runtime_parallel_counters_normalize_and_warnings_flag_hotspots() -> Result<(), ParseError> {
    // Deliberate misestimate supplement independent of native optimizer decisions.
    let xml = r#"<s:ShowPlanXML xmlns:s="http://schemas.microsoft.com/sqlserver/2004/07/showplan">
      <s:RelOp PhysicalOp="Hash Match" LogicalOp="Inner Join" EstimateRows="1" EstimatedTotalSubtreeCost="5">
        <s:RunTimeInformation>
          <s:RunTimeCountersPerThread ActualRows="200" ActualExecutions="2" ActualElapsedms="20"/>
          <s:RunTimeCountersPerThread ActualRows="400" ActualExecutions="2" ActualElapsedms="30"/>
        </s:RunTimeInformation>
        <s:Warnings><s:SpillToTempDb SpillLevel="1"/></s:Warnings>
      </s:RelOp><s:MissingIndexGroup Impact="80"/>
    </s:ShowPlanXML>"#;
    let plan = analyse(parse(xml)?);
    let node = &plan.nodes[0];
    assert_eq!(node.data.actual_rows, Some(300.0));
    assert_eq!(node.data.loops, Some(2.0));
    assert_eq!(node.inclusive_ms, Some(30.0));
    assert!(
        node.hotspots
            .iter()
            .any(|hotspot| matches!(hotspot, Hotspot::Spill { .. }))
    );
    assert!(
        node.hotspots
            .iter()
            .any(|hotspot| matches!(hotspot, Hotspot::Misestimate { .. }))
    );
    assert_eq!(plan.summary[0].label, "Missing index");
    Ok(())
}

#[test]
fn rejects_malformed_oversized_and_deep_xml() {
    for xml in [
        "<ShowPlanXML><RelOp>",
        "<!DOCTYPE x><ShowPlanXML/>",
        "<x/>",
        "<ShowPlanXML><RelOp PhysicalOp='Scan'/></wrong>",
    ] {
        assert!(parse(xml).is_err(), "{xml}");
    }
    assert!(parse(&"x".repeat(MAX_BYTES + 1)).is_err());
    assert!(
        parse(&format!(
            "{}<x/>{}",
            "<x>".repeat(MAX_DEPTH),
            "</x>".repeat(MAX_DEPTH)
        ))
        .is_err()
    );
    assert!(
        parse(&format!(
            "{}{}",
            "<x>".repeat(MAX_DEPTH + 1),
            "</x>".repeat(MAX_DEPTH + 1)
        ))
        .is_err()
    );
}

#[test]
fn missing_timings_use_cost_and_entities_are_decoded() -> Result<(), ParseError> {
    let xml = r#"<ShowPlanXML><RelOp PhysicalOp="Index Seek" EstimatedTotalSubtreeCost="NaN">
      <Object Schema="[dbo]" Table="[a&amp;b]" Index="[i]]x]"/>
      <RunTimeCountersPerThread ActualRows="4" ActualExecutions="2"/>
      <ScalarOperator ScalarString="[x]&lt;5"/>
    </RelOp></ShowPlanXML>"#;
    let plan = parse(xml)?;
    assert_eq!(plan.root.data.relation.as_deref(), Some("dbo.a&b"));
    assert_eq!(plan.root.data.index.as_deref(), Some("i]x"));
    assert_eq!(plan.root.data.actual_total_ms, None);
    assert_eq!(plan.root.data.total_cost, None);
    assert_eq!(plan.root.data.conditions[0].value, "[x]<5");
    Ok(())
}
