// SPDX-License-Identifier: Apache-2.0
use super::*;
use proptest::prelude::*;
use std::fmt::Write as _;
use url::form_urlencoded::Serializer;

const ID: &str = "0192d3c4-5b6a-7c8d-9e0f-a1b2c3d4e5f6";

fn open(text: &str) -> Result<OpenLinkDto, LinkError> {
    match parse(text)? {
        ParsedLink::Open(link) => Ok(link),
        ParsedLink::File(_) => Err(LinkError::Action),
    }
}

fn by_name(name: &str) -> LinkConnectionDto {
    LinkConnectionDto {
        id: None,
        name: Some(name.to_owned()),
    }
}

#[test]
fn connection_links_name_a_profile_by_id_or_name() -> Result<(), LinkError> {
    assert_eq!(
        open("openrows://connection?name=Local%20PG&database=app")?,
        OpenLinkDto::Connection {
            connection: by_name("Local PG"),
            database: Some("app".to_owned()),
        }
    );
    let link = open(&format!("openrows://connection/?id={ID}&name=x"))?;
    assert!(
        matches!(link, OpenLinkDto::Connection { connection, database: None }
        if connection.id.as_deref() == Some(ID))
    );
    assert_eq!(
        open("openrows://connection"),
        Err(LinkError::Missing("name"))
    );
    assert_eq!(
        open("openrows://connection?id=nope"),
        Err(LinkError::Invalid("id"))
    );
    Ok(())
}

#[test]
fn object_and_table_links_carry_parameterised_filters_only() -> Result<(), LinkError> {
    assert_eq!(
        open("openrows://object?name=pg&database=app&schema=sales")?,
        OpenLinkDto::Object {
            connection: by_name("pg"),
            database: "app".to_owned(),
            schema: "sales".to_owned(),
            object: None,
        }
    );
    let link = open(
        "openrows://table?name=pg&database=app&schema=public&object=orders&kind=view\
         &filter=column%3Dstatus%26op%3Dequal%26value%3Dpaid%2C%20late\
         &filter=column%3Dtotal%26op%3Dbetween%26value%3D1%26second%3D9",
    )?;
    let OpenLinkDto::Table {
        object, filters, ..
    } = link
    else {
        return Err(LinkError::Action);
    };
    assert_eq!(object.kind, ObjectKindDto::View);
    assert_eq!(filters.len(), 2);
    assert_eq!(filters[0].value, "paid, late");
    assert_eq!(filters[1].operator, FilterOperatorDto::Between);
    assert_eq!(filters[1].second, "9");
    let raw = "openrows://table?name=pg&database=a&schema=s&object=t\
               &filter=column%3Dx%26op%3Draw%26value%3D1%3D1";
    assert_eq!(open(raw), Err(LinkError::RawFilter));
    assert_eq!(
        open("openrows://table?name=pg&database=a&schema=s"),
        Err(LinkError::Missing("object"))
    );
    Ok(())
}

#[test]
fn file_links_hand_off_only_associated_absolute_paths() {
    assert_eq!(
        parse("openrows://file?path=%2Ftmp%2Freport.SQL"),
        Ok(ParsedLink::File(PathBuf::from("/tmp/report.SQL")))
    );
    for bad in [
        "openrows://file?path=relative.sql",
        "openrows://file?path=%2Fbin%2Fsh",
        "openrows://file?path=%2Ftmp%2Fx",
    ] {
        assert_eq!(parse(bad), Err(LinkError::Invalid("path")), "{bad}");
    }
}

#[test]
fn hostile_links_are_rejected_with_typed_errors() {
    let cases = [
        ("https://example.com", LinkError::Scheme),
        (
            "openrows://user:hunter2@connection?name=x",
            LinkError::Secret,
        ),
        (
            "openrows://connection?name=x&password=hunter2",
            LinkError::Secret,
        ),
        ("openrows://connection?name=x&apiKey=1", LinkError::Secret),
        (
            "openrows://connection?name=postgres%3A%2F%2Fu%3Ap%40h%2Fdb",
            LinkError::Secret,
        ),
        ("openrows://connection?name=x&sql=DROP", LinkError::Unknown),
        ("openrows://connection?name=x&name=y", LinkError::Duplicate),
        ("openrows://connection?name=a%00b", LinkError::Encoding),
        ("openrows://connection?name=a%FFb", LinkError::Encoding),
        (
            "openrows://connection?name=a%0Ab",
            LinkError::Invalid("name"),
        ),
        ("openrows://connection?name=", LinkError::Invalid("name")),
        ("openrows://run?sql=DROP%20TABLE%20x", LinkError::Action),
        ("openrows://connection:5432?name=x", LinkError::Malformed),
        ("openrows://connection/extra?name=x", LinkError::Malformed),
        ("openrows://connection?name=x#frag", LinkError::Malformed),
        (
            "openrows://object?name=x&database=d&schema=s&kind=function&object=f",
            LinkError::Invalid("kind"),
        ),
        ("not a link", LinkError::Malformed),
    ];
    for (text, error) in cases {
        assert_eq!(parse(text).err(), Some(error), "{text}");
    }
    assert_eq!(
        parse(&format!("openrows://connection?name={}", "a".repeat(9000))),
        Err(LinkError::TooLong)
    );
    let many = "&filter=column%3Dc%26op%3DisNull".repeat(17);
    assert_eq!(
        parse(&format!(
            "openrows://table?name=x&database=d&schema=s&object=t{many}"
        )),
        Err(LinkError::TooManyFilters)
    );
}

/// The exact text `linkText.test.ts` expects the webview's Copy Link to write.
#[test]
fn webview_copy_links_parse() -> Result<(), LinkError> {
    let text = "openrows://table?id=0192d3c4-5b6a-7c8d-9e0f-a1b2c3d4e5f6&name=Local+PG\
                &database=app&schema=public&object=orders&kind=view\
                &filter=column%3Dstatus%26op%3Dequal%26value%3Dpaid%2B%2526%2Blate%26second%3D\
                &filter=column%3Dtotal%26op%3Dbetween%26value%3D1%26second%3D9";
    let OpenLinkDto::Table {
        connection,
        object,
        filters,
        ..
    } = open(text)?
    else {
        return Err(LinkError::Action);
    };
    assert_eq!(connection.id.as_deref(), Some(ID));
    assert_eq!(connection.name.as_deref(), Some("Local PG"));
    assert_eq!(object.kind, ObjectKindDto::View);
    assert_eq!(filters[0].value, "paid & late");
    assert_eq!(filters[1].second, "9");
    Ok(())
}

#[test]
fn the_scheme_is_registered_in_the_bundle() {
    let plist = include_str!("../../Info.plist");
    assert!(plist.contains("<key>CFBundleURLSchemes</key>"));
    assert!(plist.contains(&format!("<string>{SCHEME}</string>")));
}

#[test]
fn messages_never_repeat_link_contents() {
    let error = parse("openrows://connection?name=x&password=hunter2").err();
    assert!(error.is_some_and(|error| !error.to_string().contains("hunter2")));
}

/// Encode a link as the webview's Copy Link does.
fn encode(link: &OpenLinkDto) -> String {
    let mut query = Serializer::new(String::new());
    let (action, connection) = match link {
        OpenLinkDto::Connection { connection, .. } => ("connection", connection),
        OpenLinkDto::Object { connection, .. } => ("object", connection),
        OpenLinkDto::Table { connection, .. } => ("table", connection),
    };
    for (key, value) in [("id", &connection.id), ("name", &connection.name)] {
        if let Some(value) = value {
            query.append_pair(key, value);
        }
    }
    match link {
        OpenLinkDto::Connection { database, .. } => {
            if let Some(database) = database {
                query.append_pair("database", database);
            }
        }
        OpenLinkDto::Object {
            database,
            schema,
            object,
            ..
        } => {
            query.append_pair("database", database);
            query.append_pair("schema", schema);
            if let Some(object) = object {
                append_object(&mut query, object);
            }
        }
        OpenLinkDto::Table {
            database,
            schema,
            object,
            filters,
            ..
        } => {
            query.append_pair("database", database);
            query.append_pair("schema", schema);
            append_object(&mut query, object);
            for filter in filters {
                query.append_pair("filter", &encode_filter(filter));
            }
        }
    }
    format!("openrows://{action}?{}", query.finish())
}

fn append_object(query: &mut Serializer<'_, String>, object: &LinkObjectDto) {
    query.append_pair("object", &object.name);
    let kind = serde_json::to_value(object.kind).unwrap_or_default();
    query.append_pair("kind", kind.as_str().unwrap_or_default());
}

fn encode_filter(filter: &FilterDto) -> String {
    let operator = serde_json::to_value(filter.operator).unwrap_or_default();
    Serializer::new(String::new())
        .append_pair("column", &filter.column)
        .append_pair("op", operator.as_str().unwrap_or_default())
        .append_pair("value", &filter.value)
        .append_pair("second", &filter.second)
        .finish()
}

/// Identifiers: printable, single-line, never the replacement character.
fn name() -> impl Strategy<Value = String> {
    "[\\PC&&[^\\x{FFFD}]]{1,24}".prop_filter("no credential URLs", |text| !text.contains("://"))
}

fn operator() -> impl Strategy<Value = FilterOperatorDto> {
    prop::sample::select(vec![
        FilterOperatorDto::Equal,
        FilterOperatorDto::Contains,
        FilterOperatorDto::Between,
        FilterOperatorDto::In,
        FilterOperatorDto::IsNull,
        FilterOperatorDto::Regex,
    ])
}

fn kind() -> impl Strategy<Value = ObjectKindDto> {
    prop::sample::select(vec![
        ObjectKindDto::Table,
        ObjectKindDto::View,
        ObjectKindDto::MaterializedView,
        ObjectKindDto::ForeignTable,
    ])
}

fn filter_dto() -> impl Strategy<Value = FilterDto> {
    (name(), operator(), name(), "[\\PC\n\t&&[^\\x{FFFD}]]{0,24}").prop_map(
        |(column, operator, value, second)| FilterDto {
            column,
            operator,
            value: value.replace("://", ""),
            second: second.replace("://", ""),
        },
    )
}

fn table_link() -> impl Strategy<Value = OpenLinkDto> {
    (
        name(),
        name(),
        name(),
        name(),
        kind(),
        prop::collection::vec(filter_dto(), 0..=16),
    )
        .prop_map(
            |(connection, database, schema, object, kind, filters)| OpenLinkDto::Table {
                connection: by_name(&connection),
                database,
                schema,
                object: LinkObjectDto { name: object, kind },
                filters,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Fuzz: arbitrary text never panics and errors are typed.
    #[test]
    fn arbitrary_text_never_panics(text in "\\PC*") {
        let _ = parse(&text);
        let _ = parse(&format!("openrows://table?{text}"));
    }

    /// Fuzz: arbitrary bytes, percent-encoded into every position.
    #[test]
    fn arbitrary_encoded_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..200)) {
        let encoded = bytes.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "%{byte:02X}");
            text
        });
        for template in ["openrows://{}", "openrows://connection?name={}", "openrows://table?name=a&database=b&schema=c&object=d&filter={}"] {
            let _ = parse(&template.replace("{}", &encoded));
        }
    }

    /// Every link Copy Link can produce parses back to the same request.
    #[test]
    fn copied_links_round_trip(link in table_link()) {
        prop_assert_eq!(open(&encode(&link)), Ok(link));
    }

    /// A credential-like key anywhere is refused, whatever else the link holds.
    #[test]
    fn secret_keys_are_always_refused(link in table_link(), secret in prop::sample::select(SECRET_KEY_SAMPLES.to_vec())) {
        let text = format!("{}&{secret}=x", encode(&link));
        prop_assert_eq!(parse(&text), Err(LinkError::Secret));
    }
}

const SECRET_KEY_SAMPLES: &[&str] = &[
    "password",
    "PASS",
    "apiKey",
    "token",
    "auth",
    "clientSecret",
    "pwd",
];
