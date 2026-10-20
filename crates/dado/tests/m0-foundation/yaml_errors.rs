//! Acceptance tests for AC-011 through AC-013: error display, diag interop,
//! and scalar redaction.

use std::collections::BTreeMap;

use dado::yaml::from_str;

const DUPLICATE_SOURCE: &str = "a: 1\nb: 2\nb: 3\n";

#[test]
fn ac_011_display_includes_code_message_position_and_offset() {
    let err = from_str::<BTreeMap<String, i64>>(DUPLICATE_SOURCE).unwrap_err();

    assert_eq!(err.code, "D1001");
    assert_eq!(err.message, "duplicate mapping key");
    assert_eq!((err.line, err.column, err.offset), (3, 1, 10));
    assert_eq!(
        err.to_string(),
        "D1001: duplicate mapping key (3:1, offset 10)"
    );
    assert_eq!(format!("{err}"), err.to_string());
}

#[cfg(feature = "diag")]
#[test]
fn ac_012_diag_span_renders_caret_under_failing_construct() {
    use dado::diag::{render, Diagnostic, Severity, Span};

    let err = from_str::<BTreeMap<String, i64>>(DUPLICATE_SOURCE).unwrap_err();
    let diagnostic = Diagnostic {
        code: err.code,
        message: err.message.clone(),
        severity: Severity::Error,
        span: Some(Span::new(err.offset, err.offset + 1)),
        help: None,
    };

    let rendered = render(DUPLICATE_SOURCE, &[diagnostic]);

    assert!(rendered.contains("error D1001: duplicate mapping key"));
    assert!(rendered.contains("3 | b: 3"));
    assert!(rendered.contains("| ^"), "missing caret in:\n{rendered}");
    assert!(
        rendered
            .lines()
            .any(|line| line.contains('^') && line.ends_with("| ^")),
        "caret should sit at column 1 of the failing key:\n{rendered}"
    );
}

#[test]
fn ac_013_error_output_does_not_echo_source_or_scalar_values() {
    let duplicate =
        from_str::<BTreeMap<String, String>>("password: hunter2\npassword: hunter2\n").unwrap_err();
    let duplicate_output = format!("{duplicate} {duplicate:?}");
    assert!(!duplicate_output.contains("hunter2"));
    assert!(!duplicate_output.contains("password"));

    let non_finite =
        from_str::<BTreeMap<String, f64>>("password: hunter2\nratio: .nan\n").unwrap_err();
    let non_finite_output = format!("{non_finite} {non_finite:?}");
    assert!(!non_finite_output.contains("hunter2"));
    assert!(!non_finite_output.contains("password"));

    let type_mismatch = from_str::<BTreeMap<String, u32>>("password: hunter2\n").unwrap_err();
    let type_mismatch_output = format!("{type_mismatch} {type_mismatch:?}");
    assert!(!type_mismatch_output.contains("hunter2"));
    assert!(!type_mismatch_output.contains("password"));
}
