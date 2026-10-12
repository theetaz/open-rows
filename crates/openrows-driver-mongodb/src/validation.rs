// SPDX-License-Identifier: Apache-2.0
//! Guard and BSON identity validation before any staged write reaches the server.
use crate::error;
use mongodb::bson::{Bson, Document};
use openrows_driver_api::DriverError;
use openrows_sql::mql::Query;

pub(crate) fn guard(query: &Query) -> Result<(), DriverError> {
    let filter = error::document(&query.arguments[0])?;
    let id = filter.get("_id").ok_or_else(error::invalid)?;
    let eq = filter
        .get_document("$expr")
        .and_then(|expr| expr.get_array("$eq"))
        .map_err(|_| error::invalid())?;
    if eq.len() != 2 || eq[0] != Bson::String("$$ROOT".into()) {
        return Err(error::invalid());
    }
    let original = eq[1]
        .as_document()
        .and_then(|literal| literal.get_document("$literal").ok())
        .ok_or_else(error::invalid)?;
    if original.get("_id") != Some(id) {
        return Err(error::invalid());
    }
    if query.method == "replaceOne" {
        identity(&error::document(&query.arguments[1])?, id)?;
    }
    Ok(())
}
fn identity(replacement: &Document, id: &Bson) -> Result<(), DriverError> {
    if replacement.get("_id") == Some(id) {
        Ok(())
    } else {
        Err(DriverError::Unsupported(
            "A MongoDB document's _id cannot change".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staged_guards_require_the_whole_document_and_immutable_bson_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let query = openrows_sql::mql::parse(
            "db.x.replaceOne({_id:{$numberInt:'1'},$expr:{$eq:['$$ROOT',{$literal:{_id:{$numberInt:'1'},n:2}}]}},{_id:1,n:3})",
        )?;
        guard(&query)?;
        let changed = openrows_sql::mql::parse(
            "db.x.replaceOne({_id:1,$expr:{$eq:['$$ROOT',{$literal:{_id:1,n:2}}]}},{_id:2,n:3})",
        )?;
        assert!(guard(&changed).is_err());
        let unguarded = openrows_sql::mql::parse("db.x.deleteOne({_id:1,$expr:true})")?;
        assert!(guard(&unguarded).is_err());
        Ok(())
    }
    #[test]
    fn invalid_ejson_and_write_concern_errors_fail_without_echoing_values() {
        assert!(error::document(&serde_json::json!({"_id":{"$oid":"invalid"}})).is_err());
        assert!(
            error::document(
                &serde_json::json!({"n":{"$numberLong":"1","extra":"must not disappear"}})
            )
            .is_err()
        );
        let reply = mongodb::bson::doc! {"writeConcernError":{"errmsg":"synthetic-private-value"}};
        let error = crate::command::check_reply(&reply).err();
        assert!(matches!(error,Some(DriverError::Server{ref code,..}) if code == "08007"));
        assert!(
            !error
                .map_or_else(String::new, |error| error.to_string())
                .contains("synthetic-private-value")
        );
    }
}
