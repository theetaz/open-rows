// SPDX-License-Identifier: Apache-2.0
use super::*;
#[test]
fn shell_find_and_ejson_are_data_only() -> Result<(), ParseError> {
    let q = parse(
        "db.getCollection('orders.v2').find({id: {$oid: '0123456789abcdef01234567'}}).sort({name: -1}).limit(10).skip(2)",
    )?;
    assert_eq!(q.collection, "orders.v2");
    assert_eq!(q.limit, Some(10));
    assert_eq!(q.skip, 2);
    assert_eq!(q.classification().kind, StatementKind::Read);
    Ok(())
}
#[test]
fn unsafe_or_unknown_code_never_becomes_read() {
    for source in [
        "db.x.find({});db.x.deleteMany({})",
        "db.x.find({a: f()})",
        "db.x.find({}).map(f)",
        "db.x.aggregate([{$out: 'x'}])",
        "db.x.aggregate([{$facet:{x:[{$merge: 'x'}]}}])",
        "db.x.find({$where:'function() { return true; }'})",
        "db.x.countDocuments({$where:'function() { return true; }'})",
        "db.x.find({}).limit(-1)",
    ] {
        assert!(classify(source).is_write(), "{source}");
    }
    assert!(classify("db.x.deleteMany({})").dangerous);
    assert!(!classify("db.x.deleteMany({id: 1})").dangerous);
}
#[test]
fn bson_constructors_expand_without_evaluating_code() -> Result<(), ParseError> {
    let q = parse(
        "db.x.find({_id:ObjectId('0123456789abcdef01234567'),n:NumberLong('9007199254740993')})",
    )?;
    assert_eq!(q.arguments[0]["n"]["$numberLong"], "9007199254740993");
    assert!(parse("db.x.find({_id:ObjectId(run())})").is_err());
    Ok(())
}
#[test]
fn terminal_to_array_preserves_native_projection_semantics() -> Result<(), ParseError> {
    let q = parse("db.x.find({},{__document:1}).limit(10).toArray()")?;
    assert!(q.document_view);
    assert_eq!(q.arguments[1]["__document"], 1);
    assert!(parse("db.x.find({}).toArray().limit(10)").is_err());
    Ok(())
}
#[test]
fn all_mutations_are_writes() {
    for source in [
        "db.x.insertOne({a:1})",
        "db.x.insertMany([{a:1}])",
        "db.x.updateOne({id:1},{$set:{a:2}})",
        "db.x.updateMany({},{$set:{a:2}})",
        "db.x.deleteOne({id:1})",
        "db.x.createIndex({a:1})",
        "db.x.dropIndex('a_1')",
    ] {
        assert!(classify(source).is_write());
    }
}

#[test]
fn lossy_numbers_and_duplicate_fields_are_rejected_before_execution() {
    for source in [
        "db.x.insertOne({n:NaN})",
        "db.x.insertOne({n:Infinity})",
        "db.x.insertOne({n:1e999})",
        "db.x.insertOne({n:18446744073709551615})",
        "db.x.insertOne({n:1,n:2})",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    assert!(parse("db.x.insertOne({n:{$numberDouble:'NaN'}})").is_ok());
}
