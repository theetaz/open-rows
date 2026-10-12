// SPDX-License-Identifier: Apache-2.0
//! Admit monitor packets/rows before text allocation; discard unnamed SHOW fields.
use crate::wire::Rows;
use mysql_common::packets::Column;
use openrows_driver_api::DriverError;

#[derive(Default)]
pub(super) struct Bounded {
    pub(super) rows: Vec<Vec<Option<String>>>,
    pub(super) names: Vec<String>,
    pub(super) partial: bool,
    pub(super) select_names: Option<&'static [&'static str]>,
    selected: Vec<usize>,
    bytes: usize,
}
impl Bounded {
    pub(super) fn new(names: Option<&'static [&'static str]>) -> Self {
        Self {
            select_names: names,
            ..Self::default()
        }
    }
}
impl Rows for Bounded {
    fn begin(&mut self, columns: &[Column]) -> Result<(), DriverError> {
        if columns.len() > 128 {
            return Err(limit());
        }
        self.names = columns.iter().map(|c| c.name_str().into_owned()).collect();
        self.selected = self
            .names
            .iter()
            .enumerate()
            .filter(|(_, name)| {
                self.select_names
                    .is_none_or(|allowed| allowed.iter().any(|a| name.eq_ignore_ascii_case(a)))
            })
            .map(|(index, _)| index)
            .collect();
        self.names = self
            .selected
            .iter()
            .map(|i| self.names[*i].clone())
            .collect();
        Ok(())
    }
    fn row(&mut self, values: Vec<Option<Vec<u8>>>) -> Result<bool, DriverError> {
        self.bytes += values.iter().flatten().map(Vec::len).sum::<usize>();
        if self.rows.len() >= 1_000 || self.bytes > 1_024 * 1_024 {
            return Err(limit());
        }
        self.partial |= self.selected.iter().any(|i| {
            values
                .get(*i)
                .and_then(Option::as_ref)
                .is_some_and(|v| v.len() >= 4_096)
        });
        self.rows.push(
            self.selected
                .iter()
                .map(|index| {
                    values
                        .get(*index)
                        .and_then(Option::as_ref)
                        .filter(|v| v.len() < 4_096)
                        .map(|v| String::from_utf8_lossy(v).into_owned())
                })
                .collect(),
        );
        Ok(true)
    }
    fn end(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}
fn limit() -> DriverError {
    DriverError::Protocol("monitor result limit exceeded".into())
}
