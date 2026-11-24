// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A type parameter's `const` / `in` / `out` modifiers may be written in **any
//! order**. tsc's parser collects them with an order-free modifier loop and leaves
//! "'const' modifier must precede 'in' modifier" to its grammar checker, so the
//! ordering rule joins the static-semantic early-errors tsv defers (as the *context*
//! rule — "only on a class, interface or type alias" — already is). Prettier formats
//! every ordering.
//!
//! Both of tsv's outputs are order-blind:
//!
//! - the **wire** writes the modifier keys in one order, `const`, `in`, `out` — tsv's
//!   wire takes one key order per node type, where acorn-typescript's `tsParseModifiers`
//!   stamps each flag as it consumes the keyword, so its key order follows the source
//!   (key order is not graded against the oracle; keys, values and presence are);
//! - the **printer** emits the canonical `const in out` however the source spells it,
//!   exactly as prettier does.
//!
//! The accepts and the normalization are pinned by
//! `typescript/typescript_specific/generics/type_param_modifier_order`; the one wire for
//! every spelling is pinned here.
//!
//! ⚠️ One spelling has no fixture form at ALL, not even a variant: the **reversed
//! variance pair** `<out in T>`, which acorn-typescript *rejects* in every declaration
//! kind (it is order-free for `const` against a variance modifier, but not for `in`
//! against `out`). tsc's parser accepts it with an empty `parseDiagnostics` and raises
//! TS1029 `'in' modifier must precede 'out' modifier` from its grammar checker, so tsv
//! defers it with the rest of the family — and this file is its only pin, both for the
//! accept and for the wire keys. See `docs/conformance_svelte.md` §TypeScript
//! Corrections.

use serde_json::Value;

fn wire(source: &str) -> Vec<u8> {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    tsv_ts::__private::convert_ast_json_bytes(&program, source)
}

fn parse_json(source: &str) -> Value {
    tsv_debug::json::wire_value(&wire(source))
}

fn format(source: &str) -> String {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

/// The emitted modifier keys of the first type parameter, in wire order.
fn modifier_keys(source: &str) -> Vec<String> {
    let json = parse_json(source);
    let param = json
        .pointer("/body/0/typeParameters/params/0")
        .and_then(Value::as_object)
        .expect("a first type parameter");
    param
        .keys()
        .filter(|k| matches!(k.as_str(), "const" | "in" | "out"))
        .cloned()
        .collect()
}

/// The wire writes the modifiers present in one order, `const`, `in`, `out`, whatever
/// order the author wrote them in.
#[test]
fn wire_keys_take_one_order() {
    for (source, expected) in [
        ("class C<const in T> {}", vec!["const", "in"]),
        ("class C<in const T> {}", vec!["const", "in"]),
        ("class C<const out T> {}", vec!["const", "out"]),
        ("class C<out const T> {}", vec!["const", "out"]),
        ("class C<in out T> {}", vec!["in", "out"]),
        // The one spelling acorn rejects outright — tsv defers TS1029, so the wire
        // shape below is tsv's alone (see the module doc).
        ("class C<out in T> {}", vec!["in", "out"]),
        ("class C<const T> {}", vec!["const"]),
        ("class C<in T> {}", vec!["in"]),
        ("class C<out T> {}", vec!["out"]),
    ] {
        assert_eq!(modifier_keys(source), expected, "wire keys of {source:?}");
    }
}

/// Spellings that differ only in modifier order give byte-identical wires: the parameter's
/// span covers the modifiers either way, and no field records which came first.
#[test]
fn reordered_spellings_write_the_same_bytes() {
    for (a, b) in [
        ("class C<const in T> {}", "class C<in const T> {}"),
        ("class C<const out T> {}", "class C<out const T> {}"),
        ("class C<in out T> {}", "class C<out in T> {}"),
        ("class C<const in out T> {}", "class C<out in const T> {}"),
    ] {
        assert_eq!(wire(a), wire(b), "{a:?} and {b:?}");
    }
}

/// A parameter with no modifiers emits none of the three keys (they are omitted, not
/// emitted as `false`).
#[test]
fn wire_omits_absent_modifiers() {
    assert!(modifier_keys("class C<T> {}").is_empty());
}

/// The printer is order-blind: every spelling renders as canonical `const in out`,
/// which is what prettier emits too. So a reversed authoring is normalized on the
/// first pass and is a fixed point on the second.
#[test]
fn printer_normalizes_to_canonical_order() {
    for (source, expected) in [
        ("class C<in const T> {}\n", "class C<const in T> {}\n"),
        ("class C<out const T> {}\n", "class C<const out T> {}\n"),
        ("class C<out in T> {}\n", "class C<in out T> {}\n"),
        ("class C<const in T> {}\n", "class C<const in T> {}\n"),
        ("class C<in out T> {}\n", "class C<in out T> {}\n"),
    ] {
        let once = format(source);
        assert_eq!(once, expected, "format of {source:?}");
        assert_eq!(format(&once), once, "idempotent for {source:?}");
    }
}

/// Order-free is not repeat-free: a REPEATED modifier is a syntax error tsv raises at
/// the offending keyword, with acorn's own message.
///
/// ⚠️ This is a **deliberate rejection of input prettier formats** (it collapses the
/// repeat, printing `<out T>`), and the one place this family parts from tsc: tsc's
/// parser accepts with an empty `parseDiagnostics` and raises TS1030 `'out' modifier
/// already seen` from its grammar checker, exactly as it does the TS1029 ordering rule
/// tsv *defers*. The two are graded differently on purpose: under the reject-vs-defer
/// line (`docs/conformance_tsc.md`) a checker-raised rule only *normally* defers, and
/// rejecting the repeat keeps the line the sibling positions already draw — a repeated
/// class-member modifier (`public public foo`) rejects too.
/// acorn agrees with the verdict, so the drop-in rejection is pinned as ordinary
/// `input_invalid_*` files in
/// `typescript/typescript_specific/generics/type_param_modifier_order`; what lives here
/// is the message and the boundary below.
#[test]
fn repeated_modifier_rejects() {
    for (source, expected) in [
        ("class C<const const T> {}", "Duplicate modifier: 'const'"),
        ("class C<in in T> {}", "Duplicate modifier: 'in'"),
        ("class C<out out T> {}", "Duplicate modifier: 'out'"),
        ("class C<out out out> {}", "Duplicate modifier: 'out'"),
        (
            "class C<const in const T> {}",
            "Duplicate modifier: 'const'",
        ),
    ] {
        let err = tsv_ts::__private::parse(source)
            .expect_err(&format!("expected a parse error for {source:?}"));
        let message = err.to_string();
        assert!(
            message.contains(expected),
            "{source:?} should report {expected:?}, got {message:?}",
        );
    }
}

/// The repeat check must stay BEHIND the "can a name still follow?" guard, so a trailing
/// repeat that is really the parameter NAME is untouched: `<out out>` is variance `out`
/// on a parameter named `out`, which acorn accepts and which
/// `typescript/typescript_specific/generics/type_param_named_out` pins against its wire.
/// Getting the two in the wrong order turns that valid form into a duplicate error.
#[test]
fn trailing_repeat_is_the_name() {
    for (source, expected) in [
        ("class C<out out> {}\n", "class C<out out> {}\n"),
        ("class C<in out> {}\n", "class C<in out> {}\n"),
        ("type A<out> = out;\n", "type A<out> = out;\n"),
    ] {
        assert_eq!(format(source), expected, "format of {source:?}");
    }
    assert_eq!(
        modifier_keys("class C<out out> {}"),
        vec!["out"],
        "the second `out` is the name, so only one modifier key is emitted",
    );
}
