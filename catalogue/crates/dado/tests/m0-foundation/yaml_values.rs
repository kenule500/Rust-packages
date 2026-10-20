//! Acceptance tests for AC-001 through AC-010: values, scalars, and rejection.

use std::collections::BTreeMap;

use dado::yaml::{from_str, Error};
use serde::de::IgnoredAny;
use serde::Deserialize;

fn parse_error(src: &str) -> Error {
    from_str::<IgnoredAny>(src).expect_err("document should have been rejected")
}

#[derive(Debug, Deserialize)]
struct Server {
    name: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
    enabled: bool,
    retries: u32,
    ratio: f64,
    tags: Vec<String>,
    limits: BTreeMap<String, i64>,
    servers: Vec<Server>,
    note: String,
    literal: String,
    folded: String,
    stripped: String,
    kept: String,
    quoted: String,
    single: String,
    flow: Vec<i64>,
    empty_map: BTreeMap<String, String>,
    empty_seq: Vec<String>,
}

#[test]
fn ac_001_parses_documented_yaml_subset_into_typed_value() {
    let src = r#"
# A comment-only line is ignored.
name: gateway # An inline comment is stripped.
enabled: true
retries: 3
ratio: 0.5
tags:
  - alpha
  - beta
limits: {cpu: 2, memory: 512}
servers:
  - name: alpha
    port: 8080
  - name: beta
    port: 8081
note: first line
  second line
literal: |
  line one
  line two
folded: >
  folded one
  folded two
stripped: |-
  no newline
kept: |+
  keep

quoted: "hello # not a comment"
single: 'it''s fine'
flow: [1, 2, 3]
empty_map: {}
empty_seq: []
"#;

    let config: Config = from_str(src).expect("document should parse");

    assert_eq!(config.name, "gateway");
    assert!(config.enabled);
    assert_eq!(config.retries, 3);
    assert_eq!(config.ratio, 0.5);
    assert_eq!(config.tags, ["alpha", "beta"]);
    assert_eq!(config.limits["cpu"], 2);
    assert_eq!(config.limits["memory"], 512);
    assert_eq!(config.servers.len(), 2);
    assert_eq!(config.servers[0].name, "alpha");
    assert_eq!(config.servers[0].port, 8080);
    assert_eq!(config.servers[1].name, "beta");
    assert_eq!(config.servers[1].port, 8081);
    assert_eq!(config.note, "first line second line");
    assert_eq!(config.literal, "line one\nline two\n");
    assert_eq!(config.folded, "folded one folded two\n");
    assert_eq!(config.stripped, "no newline");
    assert_eq!(config.kept, "keep\n\n");
    assert_eq!(config.quoted, "hello # not a comment");
    assert_eq!(config.single, "it's fine");
    assert_eq!(config.flow, [1, 2, 3]);
    assert!(config.empty_map.is_empty());
    assert!(config.empty_seq.is_empty());
}

#[derive(Debug, Deserialize)]
struct Scalars {
    null_a: Option<String>,
    null_b: Option<String>,
    yes_bool: bool,
    no_bool: bool,
    decimal: i64,
    negative: i64,
    plus: i64,
    hex: u32,
    octal: u32,
    binary: u32,
    float: f64,
    negative_float: f64,
    scientific: f64,
    leading_dot: f64,
    trailing_dot: f64,
}

#[test]
fn ac_002_parses_core_scalars_into_matching_types() {
    let src = "\
null_a: null
null_b: ~
yes_bool: true
no_bool: false
decimal: 42
negative: -7
plus: +12
hex: 0x2A
octal: 0o52
binary: 0b101010
float: 3.5
negative_float: -0.25
scientific: 1e3
leading_dot: .5
trailing_dot: 5.
";

    let scalars: Scalars = from_str(src).expect("scalars should parse");

    assert_eq!(scalars.null_a, None);
    assert_eq!(scalars.null_b, None);
    assert!(scalars.yes_bool);
    assert!(!scalars.no_bool);
    assert_eq!(scalars.decimal, 42);
    assert_eq!(scalars.negative, -7);
    assert_eq!(scalars.plus, 12);
    assert_eq!(scalars.hex, 42);
    assert_eq!(scalars.octal, 42);
    assert_eq!(scalars.binary, 42);
    assert_eq!(scalars.float, 3.5);
    assert_eq!(scalars.negative_float, -0.25);
    assert_eq!(scalars.scientific, 1000.0);
    assert_eq!(scalars.leading_dot, 0.5);
    assert_eq!(scalars.trailing_dot, 5.0);
}

#[derive(Debug, Deserialize)]
struct Words {
    values: Vec<String>,
}

#[test]
fn ac_003_keeps_yaml_1_1_boolean_words_as_strings() {
    let src = "\
values:
  - yes
  - no
  - on
  - off
  - Yes
  - ON
";

    let words: Words = from_str(src).expect("words should parse");
    assert_eq!(words.values, ["yes", "no", "on", "off", "Yes", "ON"]);
}

#[test]
fn ac_004_rejects_duplicate_mapping_keys_with_position() {
    let err = parse_error("a: 1\nb: 2\na: 3\n");
    assert_eq!(err.code, "D1001");
    assert!(err.message.contains("duplicate"));
    assert_eq!(err.line, 3);
    assert_eq!(err.column, 1);
    assert_eq!(err.offset, 10);

    let err = parse_error("{a: 1, a: 2}");
    assert_eq!(err.code, "D1001");
    assert!(err.message.contains("duplicate"));
    assert_eq!(err.line, 1);
    assert_eq!(err.column, 8);
    assert_eq!(err.offset, 7);
}

#[test]
fn ac_005_rejects_tab_indentation_with_position() {
    let err = parse_error("a:\n\tb: 1\n");
    assert_eq!(err.code, "D1002");
    assert!(err.message.contains("tab"));
    assert_eq!(err.line, 2);
    assert_eq!(err.column, 1);
    assert_eq!(err.offset, 3);

    let err = parse_error("a:\n \tb: 1\n");
    assert_eq!(err.code, "D1002");
    assert_eq!(err.line, 2);
    assert_eq!(err.column, 2);
    assert_eq!(err.offset, 4);
}

#[test]
fn ac_006_rejects_unsupported_constructs_and_names_them() {
    let cases = [
        ("a: !!str value", "tags"),
        ("a: &anchor value", "anchors"),
        ("a: *anchor", "aliases"),
        ("<<: {x: 1}", "merge keys"),
        ("? key\n: value", "complex keys"),
        ("---\na: 1", "multi-document"),
        ("a: 1\n---\nb: 2", "multi-document"),
        ("a: 1\n...", "multi-document"),
        ("%YAML 1.2\n---\na: 1", "directives"),
    ];

    for (src, construct) in cases {
        let err = parse_error(src);
        assert_eq!(err.code, "D1003", "source: {src:?}");
        assert!(
            err.message.to_ascii_lowercase().contains(construct),
            "message for {src:?} must name {construct:?}, got {:?}",
            err.message
        );
        assert!(err.line >= 1, "source: {src:?}");
        assert!(err.column >= 1, "source: {src:?}");
    }
}

#[test]
fn ac_007_rejects_non_finite_numbers_in_any_casing() {
    for literal in [
        ".inf", "+.inf", "-.inf", ".Inf", ".INF", ".nan", "+.nan", "-.nan", ".NaN", ".NAN", "-.NAN",
    ] {
        let err = parse_error(literal);
        assert_eq!(err.code, "D1009", "literal: {literal}");
        assert!(
            err.message.to_ascii_lowercase().contains("non-finite"),
            "message for {literal} was {:?}",
            err.message
        );
        assert_eq!(err.line, 1);
        assert!(err.column >= 1);
    }

    let err = parse_error("value: -.NAN");
    assert_eq!(err.code, "D1009");

    let err = parse_error("ratio: 1e400");
    assert_eq!(err.code, "D1009");
    assert!(err.message.to_ascii_lowercase().contains("not finite"));
}

#[derive(Debug, Deserialize)]
struct Retries {
    #[allow(dead_code)]
    retries: u32,
}

#[test]
fn ac_008_reports_type_mismatches_instead_of_panicking() {
    let err = from_str::<Retries>("retries: nope").expect_err("type mismatch must fail");
    assert_eq!(err.code, "D1010");
    assert_eq!(err.line, 1);
    assert!(err.column > 1, "value position was not used: {err:?}");
    assert!(err.offset > 0, "value position was not used: {err:?}");

    let err = from_str::<Vec<i64>>("items: 5").expect_err("sequence mismatch must fail");
    assert_eq!(err.code, "D1010");
    assert_eq!(err.line, 1);

    let err = from_str::<String>("[1, 2]").expect_err("string mismatch must fail");
    assert_eq!(err.code, "D1010");
    assert_eq!(err.line, 1);

    let err = from_str::<u32>("-1").expect_err("negative unsigned value must fail");
    assert_eq!(err.code, "D1010");
    assert!(
        err.message.contains("out of range") || err.message.contains("non-negative"),
        "unexpected message: {:?}",
        err.message
    );

    let err = from_str::<bool>("1").expect_err("boolean mismatch must fail");
    assert_eq!(err.code, "D1010");
    assert!(err.message.contains("boolean"));
}

#[test]
fn ac_009_reports_depth_limit_without_stack_overflow() {
    let deep_flow = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    let err = parse_error(&deep_flow);
    assert_eq!(err.code, "D1006");
    assert!(err.message.to_ascii_lowercase().contains("depth limit"));
    assert_eq!(err.line, 1);

    let deep_block = format!("{}x", "- ".repeat(200));
    let err = parse_error(&deep_block);
    assert_eq!(err.code, "D1006");
    assert!(err.message.to_ascii_lowercase().contains("depth limit"));

    let shallow_flow = format!("{}0{}", "[".repeat(100), "]".repeat(100));
    from_str::<IgnoredAny>(&shallow_flow).expect("100 nesting levels should be accepted");
}

#[test]
fn ac_010_handles_empty_and_comment_only_documents() {
    assert_eq!(from_str::<Option<String>>("").unwrap(), None);
    assert_eq!(
        from_str::<Option<String>>("# only a comment\n\n").unwrap(),
        None
    );
    assert_eq!(from_str::<Option<String>>("null").unwrap(), None);
    assert_eq!(from_str::<Option<String>>("~\n").unwrap(), None);

    from_str::<()>("").expect("empty document is unit-like");
    from_str::<()>("# only a comment\n").expect("comment-only document is unit-like");
    from_str::<()>("null").expect("null is unit-like");

    let err = from_str::<String>("").expect_err("String rejects null");
    assert_eq!(err.code, "D1010");
    assert_eq!((err.line, err.column, err.offset), (1, 1, 0));

    let err = from_str::<u32>("# only a comment\n").expect_err("u32 rejects null");
    assert_eq!(err.code, "D1010");

    assert!(from_str::<BTreeMap<String, String>>("{}")
        .unwrap()
        .is_empty());
    assert!(from_str::<Vec<String>>("[]").unwrap().is_empty());
}
