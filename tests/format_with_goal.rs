//! `tsv_ts::format_with_goal` is the exact-goal twin of `tsv_ts::format`: the goal is
//! fixed, so a source only the other goal's grammar accepts is an error rather than a
//! Module-to-Script retry.

use tsv_ts::Goal;

/// Legal only as a Script: a strict-mode-free `with` statement is a Module syntax error.
const SCRIPT_ONLY: &str = "with (a) b;";
/// Legal only as a Module: a top-level `import` declaration is a Script syntax error.
const MODULE_ONLY: &str = "import x from 'y';";

#[test]
fn script_only_source_formats_under_the_script_goal() {
    let formatted = tsv_ts::format_with_goal(SCRIPT_ONLY, Goal::Script).expect("script parse");
    assert_eq!(formatted, "with (a) b;\n");
}

#[test]
fn script_only_source_is_an_error_under_the_module_goal() {
    assert!(tsv_ts::format_with_goal(SCRIPT_ONLY, Goal::Module).is_err());
}

#[test]
fn the_fallback_format_reaches_the_script_only_source() {
    assert_eq!(
        tsv_ts::format(SCRIPT_ONLY).expect("fallback"),
        tsv_ts::format_with_goal(SCRIPT_ONLY, Goal::Script).expect("script parse"),
    );
}

#[test]
fn script_only_await_identifier_formats_under_the_script_goal() {
    let source = "var await = 1;\nawait;";
    assert!(tsv_ts::format_with_goal(source, Goal::Module).is_err());
    assert_eq!(
        tsv_ts::format_with_goal(source, Goal::Script).expect("script parse"),
        "var await = 1;\nawait;\n",
    );
}

#[test]
fn module_only_source_is_an_error_under_the_script_goal() {
    assert!(tsv_ts::format_with_goal(MODULE_ONLY, Goal::Script).is_err());
    assert_eq!(
        tsv_ts::format_with_goal(MODULE_ONLY, Goal::Module).expect("module parse"),
        "import x from 'y';\n",
    );
}

#[test]
fn a_named_goal_folds_carriage_returns_like_format() {
    let source = "a;\r\nb;\r\n";
    assert_eq!(
        tsv_ts::format_with_goal(source, Goal::Module).expect("module parse"),
        "a;\nb;\n",
    );
}
