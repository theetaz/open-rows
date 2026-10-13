// SPDX-License-Identifier: Apache-2.0
//! Translate the parsed shell subset to official `MongoDB` commands.
use crate::error::{document, invalid};
use mongodb::bson::{Bson, Document, doc};
use openrows_driver_api::DriverError;
use openrows_sql::mql::Query;

pub(crate) fn command(query: &Query) -> Result<Document, DriverError> {
    let first = query.arguments.first().map(document).transpose();
    match query.method.as_str() {
        "find" => find(query),
        "aggregate" => Ok(
            doc! {"aggregate": &query.collection, "pipeline": array(query)?, "cursor": {"batchSize": 128}},
        ),
        "countDocuments" => Ok(
            doc! {"aggregate": &query.collection, "pipeline": [doc!{"$match": first?.unwrap_or_default()}, doc!{"$count": "count"}], "cursor": {}},
        ),
        "insertOne" => Ok(
            doc! {"insert": &query.collection, "documents": [first?.ok_or_else(invalid)?], "ordered": true},
        ),
        "insertMany" => {
            Ok(doc! {"insert": &query.collection, "documents": array(query)?, "ordered": true})
        }
        "updateOne" | "updateMany" | "replaceOne" => update(query),
        "deleteOne" | "deleteMany" => Ok(
            doc! {"delete": &query.collection, "deletes": [{"q": first?.ok_or_else(invalid)?, "limit": i32::from(query.method == "deleteOne")}], "ordered": true},
        ),
        "createIndex" => index(query),
        "dropIndex" => Ok(
            doc! {"dropIndexes": &query.collection, "index": query.arguments[0].as_str().ok_or_else(invalid)?},
        ),
        "getIndexes" => Ok(doc! {"listIndexes": &query.collection, "cursor": {}}),
        _ => Err(invalid()),
    }
}

fn find(query: &Query) -> Result<Document, DriverError> {
    let mut command = doc! {"find": &query.collection, "filter": query.arguments.first().map(document).transpose()?.unwrap_or_default(), "batchSize": 128};
    if let Some(projection) = query.arguments.get(1) {
        command.insert("projection", document(projection)?);
    }
    if let Some(sort) = &query.sort {
        command.insert("sort", document(sort)?);
    }
    if let Some(limit) = query.limit {
        command.insert("limit", i64::from(limit));
    }
    if query.skip > 0 {
        command.insert("skip", i64::try_from(query.skip).map_err(|_| invalid())?);
    }
    Ok(command)
}

fn array(query: &Query) -> Result<Vec<Bson>, DriverError> {
    query.arguments[0]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|value| document(value).map(Bson::Document))
        .collect()
}

fn update(query: &Query) -> Result<Document, DriverError> {
    let filter = document(&query.arguments[0])?;
    let value = document(query.arguments.get(1).ok_or_else(invalid)?)?;
    let invalid_document = if query.method == "replaceOne" {
        value.keys().next().is_some_and(|key| key.starts_with('$'))
    } else {
        value.is_empty() || !value.keys().all(|key| key.starts_with('$'))
    };
    if invalid_document {
        return Err(invalid());
    }
    Ok(
        doc! {"update": &query.collection, "updates": [{"q": filter, "u": value, "multi": query.method == "updateMany"}], "ordered": true},
    )
}

fn index(query: &Query) -> Result<Document, DriverError> {
    let keys = document(&query.arguments[0])?;
    let mut options = query
        .arguments
        .get(1)
        .map(document)
        .transpose()?
        .unwrap_or_default();
    if options.contains_key("key") {
        return Err(invalid());
    }
    if !options.contains_key("name") {
        options.insert(
            "name",
            keys.iter()
                .map(|(key, value)| format!("{key}_{value}"))
                .collect::<Vec<_>>()
                .join("_"),
        );
    }
    options.insert("key", keys);
    Ok(doc! {"createIndexes": &query.collection, "indexes": [options]})
}

pub(crate) fn check_reply(reply: &Document) -> Result<(), DriverError> {
    check_flags(
        reply.contains_key("writeConcernError"),
        reply.contains_key("writeErrors"),
    )
}

pub(crate) fn check_raw_reply(reply: &mongodb::bson::RawDocument) -> Result<(), DriverError> {
    check_flags(
        reply
            .get("writeConcernError")
            .map_err(|_| invalid())?
            .is_some(),
        reply.get("writeErrors").map_err(|_| invalid())?.is_some(),
    )
}

fn check_flags(concern: bool, rejected: bool) -> Result<(), DriverError> {
    if concern {
        return Err(DriverError::Server {
            code: "08007".into(),
            message: "MongoDB write concern failed; the write outcome is uncertain".into(),
        });
    }
    if rejected {
        return Err(DriverError::Server { code: "HY000".into(), message: "MongoDB rejected a write; a multi-document command may have applied earlier changes".into() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_update_and_replacement_semantics_are_not_interchanged()
    -> Result<(), Box<dyn std::error::Error>> {
        for source in [
            "db.x.updateOne({},{})",
            "db.x.updateMany({},{})",
            "db.x.replaceOne({},{$set:{n:1}})",
        ] {
            assert!(command(&openrows_sql::mql::parse(source)?).is_err());
        }
        command(&openrows_sql::mql::parse(
            "db.x.updateOne({},{$set:{n:1}})",
        )?)?;
        command(&openrows_sql::mql::parse("db.x.replaceOne({},{})")?)?;
        Ok(())
    }
}
