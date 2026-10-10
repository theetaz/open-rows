// SPDX-License-Identifier: Apache-2.0
//! Bounded streaming showplan XML; namespace prefixes do not affect the model.
mod fields;
use crate::{Detail, NodeData, ParseError, Plan, PlanNode};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::BTreeMap;

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
const MAX_NODES: usize = 10_000;

type Attributes = BTreeMap<String, String>;

#[derive(Default)]
struct Builder {
    operators: Vec<Operator>,
    roots: Vec<PlanNode>,
    summary: Vec<Detail>,
    analyzed: bool,
    execution_ms: Option<f64>,
    nodes: usize,
    documents: usize,
}

struct Operator {
    node: PlanNode,
    rows: f64,
    executions: f64,
    elapsed_ms: Option<f64>,
}

pub(super) fn parse(xml: &str) -> Result<Plan, ParseError> {
    if xml.len() > MAX_BYTES {
        return Err(invalid("input exceeds 8 MiB"));
    }
    let mut reader = Reader::from_str(xml);
    let mut builder = Builder::default();
    let mut depth = 0_usize;
    loop {
        match reader
            .read_event()
            .map_err(|error| invalid(&error.to_string()))?
        {
            Event::Start(element) => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(invalid("nesting exceeds 128 elements"));
                }
                builder.start(&element)?;
            }
            Event::Empty(element) => {
                if depth >= MAX_DEPTH {
                    return Err(invalid("nesting exceeds 128 elements"));
                }
                builder.start(&element)?;
                builder.end(element.local_name().as_ref())?;
            }
            Event::End(element) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| invalid("unexpected end"))?;
                builder.end(element.local_name().as_ref())?;
            }
            Event::DocType(_) => return Err(invalid("DTDs are not supported")),
            Event::Eof => break,
            _ => {}
        }
    }
    if depth != 0 || !builder.operators.is_empty() {
        return Err(invalid("unclosed element"));
    }
    builder.finish()
}

impl Builder {
    fn start(&mut self, element: &BytesStart<'_>) -> Result<(), ParseError> {
        let name = element.local_name();
        let attrs = attributes(element)?;
        match name.as_ref() {
            "ShowPlanXML" => self.documents += 1,
            "RelOp" => self.operator(&attrs)?,
            "QueryTimeStats" => self.execution_ms = fields::number(&attrs, "ElapsedTime"),
            "RunTimeCountersPerThread" => self.runtime(&attrs),
            "MissingIndex" | "MissingIndexGroup" => {
                self.summary
                    .push(Detail::new("Missing index", fields::describe(&attrs)));
            }
            _ => self.detail(name.as_ref(), &attrs),
        }
        Ok(())
    }

    fn operator(&mut self, attrs: &Attributes) -> Result<(), ParseError> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(invalid("more than 10000 operators"));
        }
        let node_type = attrs
            .get("PhysicalOp")
            .ok_or(ParseError::Missing("PhysicalOp"))?;
        let mut data = NodeData::new(node_type);
        data.estimated_rows = fields::number(attrs, "EstimateRows");
        data.total_cost = fields::number(attrs, "EstimatedTotalSubtreeCost");
        data.full_scan = matches!(
            node_type.as_str(),
            "Table Scan" | "Index Scan" | "Clustered Index Scan"
        );
        data.details = attrs
            .iter()
            .map(|(key, value)| Detail::new(key, value))
            .collect();
        self.operators.push(Operator {
            node: PlanNode::leaf(data),
            rows: 0.0,
            executions: 0.0,
            elapsed_ms: None,
        });
        Ok(())
    }

    fn runtime(&mut self, attrs: &Attributes) {
        let Some(operator) = self.operators.last_mut() else {
            return;
        };
        self.analyzed = true;
        operator.rows += fields::number(attrs, "ActualRows").unwrap_or(0.0);
        // Parallel workers run together: use max loops/time, sum rows across workers.
        operator.executions = operator
            .executions
            .max(fields::number(attrs, "ActualExecutions").unwrap_or(0.0));
        if let Some(ms) = fields::number(attrs, "ActualElapsedms") {
            operator.elapsed_ms = Some(operator.elapsed_ms.unwrap_or(0.0).max(ms));
        }
        let data = &mut operator.node.data;
        data.loops = Some(operator.executions);
        let divisor = operator.executions.max(1.0);
        data.actual_rows = Some(operator.rows / divisor);
        data.actual_total_ms = operator.elapsed_ms.map(|ms| ms / divisor);
        if let Some(rows) = fields::number(attrs, "ActualRowsRead") {
            data.examined_rows = Some(data.examined_rows.unwrap_or(0.0) + rows);
        }
    }

    fn detail(&mut self, name: &str, attrs: &Attributes) {
        let Some(operator) = self.operators.last_mut() else {
            return;
        };
        let data = &mut operator.node.data;
        match name {
            "Object" => fields::object(data, attrs),
            "ScalarOperator" => {
                if let Some(value) = attrs.get("ScalarString") {
                    data.conditions.push(Detail::new("Expression", value));
                }
            }
            "SpillToTempDb" | "HashSpillDetails" | "SortSpillDetails" => {
                data.spill = Some(format!("{}: {}", name, fields::describe(attrs)));
            }
            "Warnings" | "PlanAffectingConvert" | "ColumnsWithNoStatistics" => {
                data.details
                    .push(Detail::new(name, fields::describe(attrs)));
            }
            _ => {}
        }
    }

    fn end(&mut self, name: &str) -> Result<(), ParseError> {
        if name != "RelOp" {
            return Ok(());
        }
        let operator = self
            .operators
            .pop()
            .ok_or_else(|| invalid("unexpected RelOp end"))?;
        match self.operators.last_mut() {
            Some(parent) => parent.node.children.push(operator.node),
            None => self.roots.push(operator.node),
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Plan, ParseError> {
        if self.documents == 0 || self.roots.is_empty() {
            return Err(ParseError::Missing("ShowPlanXML RelOp"));
        }
        let root = if self.roots.len() == 1 {
            self.roots.remove(0)
        } else {
            PlanNode {
                data: NodeData::new("Query plan"),
                children: self.roots,
            }
        };
        Ok(Plan {
            root,
            analyzed: self.analyzed,
            planning_ms: None,
            execution_ms: self.execution_ms,
            summary: self.summary,
        })
    }
}

fn attributes(element: &BytesStart<'_>) -> Result<Attributes, ParseError> {
    element
        .attributes()
        .map(|attr| {
            let attr = attr.map_err(|error| invalid(&error.to_string()))?;
            let key = attr.key.local_name().as_ref().to_owned();
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| invalid(&error.to_string()))?
                .into_owned();
            Ok((key, value))
        })
        .collect()
}

fn invalid(message: &str) -> ParseError {
    ParseError::Xml(message.to_owned())
}

#[cfg(test)]
#[path = "sqlserver_tests.rs"]
mod tests;
