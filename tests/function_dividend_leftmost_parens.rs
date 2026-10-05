// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The pair a function or class expression takes before a `/`, at the positions and
//! spellings its fixture does not carry.
//!
//! No function or class body's `}` prints directly before a `/` operator: the expression
//! takes a pair of its own where it is the `/`'s left operand (`(function () {}) / 2`), and
//! where that left operand ends on it (`!(function () {}) / 2`).
//!
//! The dividend's pair meets two positions that parenthesize a value which would open on the
//! `function` / `class` keyword — an expression statement, around the keyword's node, and
//! `export default`, around the whole value — and each has to see that pair, or the dividend
//! is wrapped twice. A format-ignore freeze is the other half: a verbatim slice has the pair
//! only where the author wrote it, so the same two positions read the slice rather than the
//! printed form.
//!
//! The common forms are fixture-pinned
//! (`svelte/expressions/division_after_function_parens_prettier_divergence`, and the frozen
//! bare slice in `typescript/statements/expression_statement_prettier_ignore_head_prettier_divergence`).

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

#[test]
fn export_default_adds_no_pair_around_a_parenthesized_dividend() {
    for fixed in [
        "export default (function () {}) / 2;\n",
        "export default (class {}) / 2 / 3;\n",
        "export default (async function () {}) / 2 + 1;\n",
        "export default (function () {}) / 2 ? a : b;\n",
        // through the wrappers the walk descends: a cast, a non-null, a member, a call
        "export default ((function () {}) / 2) as any;\n",
        "export default ((function () {}) / 2)!;\n",
        "export default ((function () {}) / 2).x;\n",
        "export default ((function () {}) / 2)();\n",
    ] {
        assert_eq!(format(fixed), fixed);
    }
    // The bare authoring trades the value's pair for the dividend's.
    assert_eq!(
        format("export default (function () {} / 2);\n"),
        "export default (function () {}) / 2;\n"
    );
}

#[test]
fn export_default_still_wraps_a_value_that_opens_on_the_keyword() {
    // Only a `/`'s direct left operand takes a pair of its own: under another operator, or
    // a member between the function and the `/`, the value opens on `function`.
    for fixed in [
        "export default (function () {} * 2);\n",
        "export default (function () {}.x / 2);\n",
        "export default (function () {}! / 2);\n",
    ] {
        assert_eq!(format(fixed), fixed);
    }
}

#[test]
fn a_frozen_division_takes_a_shell_only_where_its_slice_opens_on_the_keyword() {
    // The slice is the author's bytes: a bare dividend opens it on `function`, so the
    // position's own pair goes around the whole slice.
    for fixed in [
        "(\n\t// prettier-ignore\n\tfunction () {}   / 2\n);\n",
        "export default\n\t// prettier-ignore\n\t(function () {}   / 2);\n",
    ] {
        assert_eq!(format(fixed), fixed);
    }
    assert_eq!(
        format("export default (\n\t// prettier-ignore\n\tfunction () {}   / 2\n);\n"),
        "export default\n\t// prettier-ignore\n\t(function () {}   / 2);\n"
    );
    // A dividend the author parenthesized opens the slice on that `(`: no shell, so the
    // statement's redundant pair drops and the directive leads it.
    for fixed in [
        "// prettier-ignore\n(function () {})   / 2;\n",
        "export default\n\t// prettier-ignore\n\t(function () {})   / 2;\n",
    ] {
        assert_eq!(format(fixed), fixed);
    }
    assert_eq!(
        format("(\n\t// prettier-ignore\n\t(function () {})   / 2\n);\n"),
        "// prettier-ignore\n(function () {})   / 2;\n"
    );
    assert_eq!(
        format("export default (\n\t// prettier-ignore\n\t(class {})   / 2\n);\n"),
        "export default\n\t// prettier-ignore\n\t(class {})   / 2;\n"
    );
}

#[test]
fn a_freeze_of_the_dividend_alone_is_a_fixed_point_under_export_default() {
    // The directive sits in the pair the parser erased from the dividend, so pass 1 freezes
    // the function and prints the directive ahead of the value — where pass 2 reads it as
    // the value's own and freezes the whole division, pair included.
    let once = format("export default (\n\t// prettier-ignore\n\tfunction () {}\n) / 2;\n");
    assert_eq!(
        once,
        "export default\n\t// prettier-ignore\n\t(function () {}) / 2;\n"
    );
    assert_eq!(format(&once), once);
}

#[test]
fn an_operand_the_dividend_ends_on_takes_the_pair() {
    for (bare, paired) in [
        (
            "x = void function () {} / 2;\n",
            "x = void (function () {}) / 2;\n",
        ),
        ("x = delete class {} / 2;\n", "x = delete (class {}) / 2;\n"),
        ("x = +function () {} / 2;\n", "x = +(function () {}) / 2;\n"),
        (
            "x = ~async function () {} / 2;\n",
            "x = ~(async function () {}) / 2;\n",
        ),
        (
            "x = !!function () {} / 2;\n",
            "x = !!(function () {}) / 2;\n",
        ),
        (
            "x = <T>!function () {} / 2;\n",
            "x = <T>!(function () {}) / 2;\n",
        ),
        ("x = a / <T>class {} / 2;\n", "x = a / <T>(class {}) / 2;\n"),
        (
            "x = a / b / function () {} / 2;\n",
            "x = a / b / (function () {}) / 2;\n",
        ),
        (
            "x = !function () {} / !class {} / 2;\n",
            "x = !(function () {}) / !(class {}) / 2;\n",
        ),
        (
            "x = a / function* f() {} / 2;\n",
            "x = a / (function* f() {}) / 2;\n",
        ),
        // in a call's arguments and an array's elements, whose chains the other collector reads
        (
            "x = f(a / function () {} / 2);\n",
            "x = f(a / (function () {}) / 2);\n",
        ),
        (
            "x = [!function () {} / 2, a / class {} / 2];\n",
            "x = [!(function () {}) / 2, a / (class {}) / 2];\n",
        ),
    ] {
        assert_eq!(format(bare), paired, "{bare:?}");
        assert_eq!(format(paired), paired, "{paired:?}");
    }
}

#[test]
fn an_operand_that_ends_on_a_paren_stays_bare() {
    // A left operand that takes a pair of its own ends on its `)`, and with no `/` after
    // the body there is nothing to keep it from.
    for fixed in [
        "x = (a * function () {}) / 2;\n",
        "x = (a % class {}) / 2;\n",
        "x = a ** (b ** function () {}) / 2;\n",
        "x = !(<T>function () {}) / 2;\n",
        "x = (a + function () {}) / 2;\n",
        "x = (c ? a : function () {}) / 2;\n",
        "x = !function () {};\n",
        "x = !function () {} * 2;\n",
        "x = a / function () {};\n",
        "x = !function () {}.y / 2;\n",
        "x = !(function () {})() / 2;\n",
        "x /= !function () {};\n",
    ] {
        assert_eq!(format(fixed), fixed);
    }
}

#[test]
fn the_pair_keeps_the_comment_its_operand_owns() {
    // A block comment glued to the operand stays inside the pair; one after the operand
    // prints past the `)`, and a `//` leads the pair — as at every binary operand's pair.
    for fixed in [
        "(/* c */ function () {}) / 2;\n",
        "x = (/* c */ function () {}) / 2;\n",
        "x = (function () {}) /* c */ / 2;\n",
        "export default (/* c */ function () {}) / 2;\n",
        "x = a / (/* c */ function () {}) / 2;\n",
        "x = a / (function () {}) /* c */ / 2;\n",
        "x = <T>(/* c */ function () {}) / 2;\n",
        "x =\n\t// c\n\t(function () {}) / 2;\n",
    ] {
        assert_eq!(format(fixed), fixed);
    }
    for (authored, formatted) in [
        (
            "x = (function () {} /* c */) / 2;\n",
            "x = (function () {}) /* c */ / 2;\n",
        ),
        (
            "x = a / (function () {} /* c */) / 2;\n",
            "x = a / (function () {}) /* c */ / 2;\n",
        ),
        (
            "x = a / /* c */ function () {} / 2;\n",
            "x = a / (/* c */ function () {}) / 2;\n",
        ),
        (
            "x = (// c\nfunction () {}) / 2;\n",
            "x =\n\t// c\n\t(function () {}) / 2;\n",
        ),
        // A `//` after an operand that ends its own chain, inside the pair the author
        // wrote: the pair is the operand's now, and opens around the run it holds.
        (
            "x = a ** (function () {} // c\n) / 2;\n",
            "x =\n\ta **\n\t\t(\n\t\t\tfunction () {} // c\n\t\t) /\n\t2;\n",
        ),
    ] {
        assert_eq!(format(authored), formatted, "{authored:?}");
        assert_eq!(format(formatted), formatted, "{formatted:?}");
    }
}

#[test]
fn a_comment_shell_is_the_pair_an_operand_under_it_needs() {
    // A prefix operator's operand that carries a comment prints in a shell of its own, and
    // an assertion keeps the author's shell around a trailing one: either closes on a `)`
    // ahead of the `/`, so the operand takes no second pair inside it.
    for fixed in [
        "x = !(/* c */ function () {}) / 2;\n",
        "x = !(function () {} /* c */) / 2;\n",
        "x = !(/* c */ function () {} /* d */) / 2;\n",
        "x = <T>(function () {} /* c */) / 2;\n",
        "x =\n\t!( // c\n\t\tfunction () {}\n\t) / 2;\n",
        // a multi-line block the operand owns opens the shell, with or without the `/`
        "x =\n\t!(\n\t\t/* c\n\t\t */ function () {}\n\t) / 2;\n",
        "x =\n\ttypeof (\n\t\t/* c\n\t\t */ function () {}\n\t) / 2;\n",
    ] {
        assert_eq!(format(fixed), fixed);
    }
    assert_eq!(
        format("x = !/* c */ function () {} / 2;\n"),
        "x = !(/* c */ function () {}) / 2;\n"
    );
}

#[test]
fn a_decorated_class_the_dividend_ends_on_takes_the_pair_broken_open() {
    for (authored, formatted) in [
        (
            "x = !(@dec class {}) / 2;\n",
            "x =\n\t!(\n\t\t@dec\n\t\tclass {}\n\t) / 2;\n",
        ),
        (
            "x = <T>(@dec class {}) / 2;\n",
            "x =\n\t<T>(\n\t\t@dec\n\t\tclass {}\n\t) / 2;\n",
        ),
        (
            "x = a / (@dec class {}) / 2;\n",
            "x =\n\ta /\n\t(\n\t\t@dec\n\t\tclass {}\n\t) /\n\t2;\n",
        ),
    ] {
        assert_eq!(format(authored), formatted, "{authored:?}");
        assert_eq!(format(formatted), formatted, "{formatted:?}");
    }
}
