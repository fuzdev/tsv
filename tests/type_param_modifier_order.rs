// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A type parameter's `const` / `in` / `out` modifiers may be written in **any
//! order**. tsc's parser collects them with an order-free modifier loop and leaves
//! "'const' modifier must precede 'in' modifier" to its grammar checker, so the
//! ordering rule joins the static-semantic early-errors tsv defers (as the *context*
//! rule — "only on a class, interface or type alias" — already is). Prettier formats
//! every ordering.
//!
//! A REPEATED modifier is deferred alike (`<const const T>`; TS1030 for `in`/`out` is
//! the same grammar checker's, and a repeated `const` draws no tsc diagnostic at all).
//!
//! Both of tsv's outputs are order-blind and repeat-blind:
//!
//! - the **wire** writes the modifier keys in one order, `const`, `in`, `out` — tsv's
//!   wire takes one key order per node type, where acorn-typescript's `tsParseModifiers`
//!   stamps each flag as it consumes the keyword, so its key order follows the source
//!   (key order is not graded against the oracle; keys, values and presence are);
//! - the **printer** emits the canonical `const in out`, each modifier once, however the
//!   source spells it, exactly as prettier does.
//!
//! The accepts and the normalization are pinned by
//! `typescript/typescript_specific/generics/type_param_modifier_order`; the one wire for
//! every spelling is pinned here.
//!
//! ⚠️ Two spellings have no `input.*` form, since neither is a formatting fixed point and
//! acorn-typescript *rejects* both, so their normalization rides the fixture's variants
//! (which only the two formatters read) and their wire is pinned here alone:
//!
//! - the **reversed variance pair** `<out in T>`, refused by acorn in every declaration
//!   kind (it is order-free for `const` against a variance modifier, but not for `in`
//!   against `out`) — tsc's parser accepts it with an empty `parseDiagnostics` and raises
//!   TS1029 `'in' modifier must precede 'out' modifier` from its grammar checker
//!   (`unformatted_reversed_modifier_order`);
//! - a **repeat** (`unformatted_repeated_modifiers`).
//!
//! See `docs/conformance_svelte.md` §TypeScript Corrections.

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

/// Order-free is repeat-free too: a REPEATED modifier is accepted and folds into the
/// set, so its wire carries the single spelling's modifier keys and the printer prints
/// the modifier once — which is what prettier emits.
///
/// tsc's parser accepts every repeat with an empty `parseDiagnostics`; its grammar
/// checker raises TS1030 `'in' modifier already seen` / `'out' modifier already seen`,
/// and a repeated `const` draws no diagnostic at all. So the repeat is deferred with the
/// TS1029 ordering rule (`docs/conformance_tsc.md` §The reject-vs-defer line).
/// acorn-typescript rejects every repeat (`Duplicate modifier: 'out'`), so the accept is
/// an over-acceptance cataloged in `docs/conformance_svelte.md` §TypeScript Corrections;
/// the normalization is pinned by the `unformatted_repeated_modifiers` variant of
/// `typescript/typescript_specific/generics/type_param_modifier_order`, and the wire
/// here.
#[test]
fn repeated_modifier_folds_into_the_set() {
    for (source, single) in [
        ("class C<const const T> {}\n", "class C<const T> {}\n"),
        ("class C<in in T> {}\n", "class C<in T> {}\n"),
        ("class C<out out T> {}\n", "class C<out T> {}\n"),
        ("class C<out out out> {}\n", "class C<out out> {}\n"),
        ("class C<const in const T> {}\n", "class C<const in T> {}\n"),
        ("class C<in out in out T> {}\n", "class C<in out T> {}\n"),
        (
            "function f<const const T>() {}\n",
            "function f<const T>() {}\n",
        ),
    ] {
        assert_eq!(
            modifier_keys(source),
            modifier_keys(single),
            "{source:?} writes the modifier keys {single:?} does",
        );
        let once = format(source);
        assert_eq!(once, single, "format of {source:?}");
        assert_eq!(format(&once), once, "idempotent for {source:?}");
    }
}

/// The "can a name still follow?" guard runs on every modifier, a repeat included, so a
/// trailing repeat that is really the parameter NAME stays the name: `<out out>` is
/// variance `out` on a parameter named `out`, which acorn accepts and which
/// `typescript/typescript_specific/generics/type_param_named_out` pins against its wire.
/// Folding a repeat ahead of that guard would swallow the name and reject the valid form.
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
