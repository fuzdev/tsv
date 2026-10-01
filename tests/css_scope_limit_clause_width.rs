//! The width boundary of an `@scope` clause: the limit clause stays inside its parens
//! while the line, with the `) {` that follows the clause, is at most the print width,
//! and opens one column past it.
//!
//! Pinned here rather than as a fixture because prettier never wraps an `@scope`
//! prelude (it keeps the 101-column line), so the two formatters disagree past the
//! boundary and a plain fixture cannot hold the case. The clause reserves three columns
//! for that `) {`, and no fixture line lands on this boundary.

/// `@scope (.aaa…) to (.b > .c) {` with a root selector sized so the head line is
/// `width` columns.
fn scope_rule(width: usize) -> String {
    let fixed = "@scope (.) to (.b > .c) {".len();
    let root = "a".repeat(width - fixed);
    format!("@scope (.{root}) to (.b > .c) {{\n\tp {{\n\t\tcolor: blue;\n\t}}\n}}\n")
}

#[test]
fn a_limit_clause_ending_at_the_print_width_stays_inline() {
    let source = scope_rule(100);
    assert_eq!(source.lines().next().unwrap().len(), 100);
    assert_eq!(tsv_css::format_str(&source).unwrap(), source);
}

#[test]
fn a_limit_clause_one_column_past_the_print_width_opens() {
    let source = scope_rule(101);
    let head = source.lines().next().unwrap();
    assert_eq!(head.len(), 101);
    let opened = head.replace("to (.b > .c) {", "to (\n\t.b > .c\n) {");
    let expected = source.replacen(head, &opened, 1);
    let formatted = tsv_css::format_str(&source).unwrap();
    assert_eq!(formatted, expected);
    // The opened form is the fixed point.
    assert_eq!(tsv_css::format_str(&formatted).unwrap(), formatted);
}
