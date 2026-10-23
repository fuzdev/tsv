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
        let error = tsv_ts::parse(source, &arena).expect_err(source);
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
        assert!(tsv_ts::parse(source, &arena).is_ok(), "{source:?}");
    }
}
