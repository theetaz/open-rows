// SPDX-License-Identifier: Apache-2.0
use super::*;

const TRIGGER: (&[&str], &str) = (&["FUNCTION"], "TRIGGER");

#[test]
fn trigger_definitions_may_create_their_functions_first() -> Result<(), DefinitionError> {
    let definition = "CREATE OR REPLACE FUNCTION audit() RETURNS trigger AS $$\n\
        BEGIN NEW.at := now(); RETURN NEW; END $$ LANGUAGE plpgsql;\n\
        CREATE TRIGGER stamp BEFORE UPDATE ON t FOR EACH ROW EXECUTE FUNCTION audit();\n";
    let statements = definition_statements(definition, Dialect::Postgres, TRIGGER)?;
    assert_eq!(statements.len(), 2);
    assert!(statements[1].starts_with("CREATE TRIGGER stamp"));
    assert!(!statements[1].ends_with(';'));
    Ok(())
}

#[test]
fn definitions_refuse_other_statements() {
    let smuggled =
        "CREATE TRIGGER t AFTER INSERT ON x FOR EACH ROW EXECUTE FUNCTION f(); DROP TABLE x";
    assert_eq!(
        definition_statements(smuggled, Dialect::Postgres, TRIGGER),
        Err(DefinitionError::Unexpected {
            index: 2,
            expected: "CREATE TRIGGER".into()
        })
    );
    let no_trigger = "CREATE FUNCTION f() RETURNS int AS 'select 1' LANGUAGE sql";
    assert!(definition_statements(no_trigger, Dialect::Postgres, TRIGGER).is_err());
    assert_eq!(
        definition_statements(" ;; ", Dialect::Postgres, TRIGGER),
        Err(DefinitionError::Empty)
    );
    let routine = (&[][..], "PROCEDURE");
    let body = "CREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END";
    assert!(definition_statements(body, Dialect::MySql, routine).is_ok());
}

#[test]
fn view_queries_are_single_reads() -> Result<(), DefinitionError> {
    assert_eq!(
        view_query("  SELECT a FROM t;\n", Dialect::Postgres)?,
        "SELECT a FROM t"
    );
    assert_eq!(
        view_query("WITH x AS (SELECT 1) SELECT * FROM x", Dialect::MySql)?,
        "WITH x AS (SELECT 1) SELECT * FROM x"
    );
    for query in [
        "",
        "SELECT 1; DROP TABLE t",
        "DELETE FROM t",
        "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
    ] {
        assert_eq!(
            view_query(query, Dialect::Postgres),
            Err(DefinitionError::NotAQuery),
            "{query}"
        );
    }
    Ok(())
}

#[test]
fn routine_keywords() {
    assert_eq!(RoutineKind::Function.keyword(), "FUNCTION");
    assert_eq!(RoutineKind::Procedure.keyword(), "PROCEDURE");
}
