// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Types`]: literal round-trips of the common types and,
//! with `BindParameters`, bound text and NULL.

use openrows_driver_api::metadata::{EXTENSION_NAME, SOURCE_TYPE};
use openrows_driver_api::{Connection, Feature};
use openrows_sql::{Dialect, ParamBinder, ParamStyle};

use crate::dialect::{TypeCase, UNICODE, type_cases};
use crate::sink::{Collect, query};
use crate::{Failure, Target, ensure};

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let mut connection = target.connect().await?;
    for case in type_cases(dialect) {
        literal(connection.as_mut(), case).await?;
    }
    if target
        .driver
        .capabilities()
        .supports(Feature::BindParameters)
    {
        for value in [Some(UNICODE), Some(""), None] {
            bound(connection.as_mut(), dialect, value).await?;
        }
    }
    connection.close().await?;
    Ok(())
}

async fn literal(connection: &mut dyn Connection, case: &TypeCase) -> Result<(), Failure> {
    let sql = format!("SELECT {} AS v", case.expression);
    let (sink, _) = query(connection, &sql).await?;
    let schema = sink
        .schemas
        .first()
        .ok_or_else(|| Failure::Contract(format!("{}: no result", case.name)))?;
    let field = schema
        .fields()
        .first()
        .ok_or_else(|| Failure::Contract(format!("{}: no column", case.name)))?;
    ensure(case.arrow.matches(field.data_type()), || {
        format!(
            "{}: decoded as {}, expected {:?}",
            case.name,
            field.data_type(),
            case.arrow
        )
    })?;
    ensure(field.metadata().contains_key(SOURCE_TYPE), || {
        format!("{}: field has no {SOURCE_TYPE}", case.name)
    })?;
    if let Some(extension) = case.extension {
        let found = field.metadata().get(EXTENSION_NAME);
        ensure(found.is_some_and(|name| name == extension), || {
            format!("{}: extension {found:?}, expected {extension}", case.name)
        })?;
    }
    let value = sink.single()?;
    ensure(value.as_deref() == case.text, || {
        format!("{}: value {value:?}, expected {:?}", case.name, case.text)
    })
}

/// `SELECT <text cast of :v>` with `value` bound through [`ParamBinder`].
async fn bound(
    connection: &mut dyn Connection,
    dialect: Dialect,
    value: Option<&str>,
) -> Result<(), Failure> {
    let sql = format!("SELECT {} AS v", dialect.text_of(":v"));
    let mut binder = ParamBinder::new(dialect, ParamStyle::Named);
    let slots = binder.slots(&sql);
    let statement = binder
        .bind(&sql, &slots, |_| Some(value.map(str::to_owned)))
        .map_err(|error| Failure::Contract(error.to_string()))?;
    let mut sink = Collect::default();
    connection
        .execute_bound(&statement.sql, &statement.params, &mut sink)
        .await?;
    let echoed = sink.single()?;
    ensure(echoed.as_deref() == value, || {
        format!("bound {value:?} came back as {echoed:?}")
    })
}
