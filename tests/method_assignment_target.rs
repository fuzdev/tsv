//! A method, accessor or generator property has no form in a destructuring ASSIGNMENT target:
//! no pattern property carries one, so converting the object to a pattern would reprint the
//! method as a data property (`({ get f() {} } = x)` as `({ f: function () {} } = x)`), a
//! different program. tsv rejects it wherever an `=` target converts, as acorn does and as
//! tsc's grammar check does (TS1136).
//!
//! The fixture `typescript/expressions/destructuring/method_target` pins the rejection over
//! every position; the validator grades only that each `input_invalid_*` file fails. What
//! stays here is what a fixture cannot assert: the error's message and position, which match
//! the for-of head and the binding pattern's rejection of the same shape (both reach the
//! method's function value through the non-simple-target arm), and the compound operator,
//! whose left ecma262 never reparses as a pattern and which therefore keeps parsing.

/// Each rejection names an invalid assignment target at the method's function VALUE — its
/// parameter list's `(`, past any type parameters — the point a for-of head and a binding
/// pattern already report for the same property.
#[test]
fn method_target_rejects_at_its_value() {
    for (source, at) in [
        ("({ f() {} } = x);", "() {} }"),
        ("({ get f() {} } = x);", "() {} }"),
        ("({ set f(v) {} } = x);", "(v)"),
        ("({ async f() {} } = x);", "() {} }"),
        ("({ *f() {} } = x);", "() {} }"),
        ("({ f<T>() {} } = x);", "() {} }"),
        ("for ({ f<T>() {} } of x);", "() {} }"),
        ("[{ get f() {} }] = x;", "() {} }"),
        ("({ ...{ f() {} } } = x);", "() {} }"),
        ("({ a: { f() {} } } = x);", "() {} }"),
        ("for ({ f() {} } of xs);", "() {} }"),
        ("let { f() {} } = x;", "() {} }"),
    ] {
        let arena = bumpalo::Bump::new();
        let error = tsv_ts::__private::parse(source, &arena).expect_err(source);
        let (message, position) = (error.to_string(), error.position());
        assert!(
            message.contains("Invalid assignment target"),
            "{source:?}: {message}"
        );
        assert_eq!(
            position,
            source.find(at),
            "{source:?} points at {at:?}: {message}"
        );
    }
}

/// A compound operator's left is no pattern (ecma262 §13.15.1 reparses only the left of
/// `=`), so a method property under one is not rejected: tsc's parser accepts every case here,
/// acorn only the parenthesized ones.
#[test]
fn compound_operator_over_a_method_target_parses() {
    for source in [
        "({ f() {} } += x);",
        "({ get f() {} } -= y);",
        "[{ set f(v) {} }] ||= z;",
    ] {
        let arena = bumpalo::Bump::new();
        assert!(
            tsv_ts::__private::parse(source, &arena).is_ok(),
            "{source:?}"
        );
    }
}

/// A compound operator's left keeps its methods and accessors, and the comments inside them:
/// a comment between a method's key and its parameter list formats where it was written.
#[test]
fn compound_operator_over_a_commented_method_formats_as_written() {
    for source in [
        "({ f /* c */() {} } += x);\n",
        "({ get f /* c */() {} } -= x);\n",
        "[{ f /* c */() {} }] += x;\n",
        "f(({ async f /* c */() {} } *= y));\n",
    ] {
        assert_eq!(
            tsv_ts::format(source).expect(source),
            source,
            "{source:?} formats as written"
        );
    }
}

/// A compound operator's literal left prints its `=` value as the default prettier reads
/// there, with no pair, on every key→value arm — here the one a line comment between the key
/// and its `:` takes, where tsv keeps the comment and prettier moves it above the property.
#[test]
fn compound_target_default_value_prints_bare_past_a_key_comment() {
    let source = "({ a // c\n: b = 1 } += x);";
    let printed = "({\n\ta // c\n\t\t: b = 1\n} += x);\n";
    let out = tsv_ts::format(source).expect(source);
    assert_eq!(out, printed, "printed from {source:?}");
    assert_eq!(tsv_ts::format(&out).expect(printed), out, "a fixed point");
}

/// A comment in the pair the parser erased from a compound target's default VALUE prints where
/// the `=` pattern prints it, own-line spellings included — the trailing seam, the next
/// element's leading run and the end-of-array run all anchor where that default's doc stops.
#[test]
fn compound_target_default_value_shell_comment_prints_as_the_pattern() {
    for (compound, pattern) in [
        ("[a = (b\n/* c */), d] += x;", "[a = (b\n/* c */), d] = x;"),
        ("[a = (b\n/* c */\n)] += x;", "[a = (b\n/* c */\n)] = x;"),
        (
            "[a = (b\n/* c1 */\n) /* c2 */] += x;",
            "[a = (b\n/* c1 */\n) /* c2 */] = x;",
        ),
        (
            "({ k: a = (b\n/* c */\n), d } += x);",
            "({ k: a = (b\n/* c */\n), d } = x);",
        ),
        (
            "[[a = (b = c // c\n)]] += x;",
            "[[a = (b = c // c\n)]] = x;",
        ),
    ] {
        let out = tsv_ts::format(compound).expect(compound);
        let expected = tsv_ts::format(pattern)
            .expect(pattern)
            .replace("] = x", "] += x")
            .replace("} = x", "} += x");
        assert_eq!(out, expected, "{compound:?} prints as {pattern:?}");
        assert_eq!(
            tsv_ts::format(&out).expect(&out),
            out,
            "{compound:?}: a fixed point"
        );
    }
}

/// A `//` in a compound target's last default-value shell, and another past the `)`, stay two
/// comments on two lines.
#[test]
fn compound_target_default_value_shell_line_comments_stay_apart() {
    let source = "[a = (b // c1\n) // c2\n] += x;";
    let printed = "[\n\ta = b // c1\n\t// c2\n] += x;\n";
    let out = tsv_ts::format(source).expect(source);
    assert_eq!(out, printed, "printed from {source:?}");
    assert_eq!(tsv_ts::format(&out).expect(printed), out, "a fixed point");
}

/// A value freeze the key→`:` own-line arm does not honor (it builds the value unfrozen) leaves
/// the default's shell comment to the role's anchor, which must still print it once.
#[test]
fn compound_target_unhonored_value_freeze_keeps_the_shell_comment() {
    let source = "({\n\tk // x\n\t:\n\t\t// prettier-ignore\n\t\ta   =   (b /* c */)\n} += x);\n";
    let printed = "({\n\tk // x\n\t\t: // prettier-ignore\n\t\ta = b /* c */\n} += x);\n";
    let out = tsv_ts::format(source).expect(source);
    assert_eq!(out, printed, "printed from {source:?}");
    assert_eq!(tsv_ts::format(&out).expect(printed), out, "a fixed point");
}
