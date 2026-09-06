// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Node-type pins for a parenthesized arrow's return type inside a conditional's
//! consequent — tsc's `allowReturnTypeInArrowFunction` rule, which tsv follows
//! through its one speculative parse (`Parser::parse_arrow_or_rewind`).
//!
//! `a ? (b) : c => d` is `a ? b : (c => d)`: the `:` after the head is the
//! conditional's, and the arrow reading `(b): c => d` would leave the conditional
//! without one. tsc parses the arrow anyway and keeps the annotation only when
//! another `:` follows it (`a ? (b): c => d : e`), rewinding otherwise; acorn
//! commits on `: type =>` alone and rejects every one of the dropped shapes.
//!
//! The fixture `typescript/expressions/arrow/return_type_ternary_consequent` pins
//! the readings as formatting claims — prettier normalizes every ambiguous spelling
//! into an unambiguous one (`b ? c : (d) => e`), so no prettier fixed point is
//! acorn-rejected and the corpus spellings ride as its `unformatted_paren_placement`
//! variant. This file asserts the trees directly, on the standalone-TS path, and
//! grades what a formatting claim cannot see: that a rewound parse re-drains the
//! comments its abandoned half consumed exactly once, and that a rewound head's
//! failure inside an enclosing speculation rewinds that one too. Every expectation
//! here was graded against tsc's live parser (`createSourceFile(...).parseDiagnostics`
//! and its tree) before it was written down. Cataloged in
//! `docs/conformance_svelte.md` §TypeScript Corrections.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source))
}

fn accepts(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn comment_count(source: &str) -> usize {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    program.comments.len()
}

/// The `type` of the node at `pointer`, or `<none>` where nothing is.
fn node_type(json: &Value, pointer: &str) -> String {
    json.pointer(pointer)
        .and_then(|n| n.get("type"))
        .and_then(Value::as_str)
        .map_or_else(|| "<none>".to_owned(), str::to_owned)
}

/// Whether the arrow at `pointer` carries a `returnType`.
fn annotated(json: &Value, pointer: &str) -> bool {
    json.pointer(&format!("{pointer}/returnType"))
        .is_some_and(|t| !t.is_null())
}

const COND: &str = "/body/0/expression";
const CONSEQUENT: &str = "/body/0/expression/consequent";
const ALTERNATE: &str = "/body/0/expression/alternate";

fn check_shapes(cases: &[(&str, &str, &str)]) {
    let mut failures = Vec::new();
    for (source, consequent, alternate) in cases {
        if !accepts(source) {
            failures.push(format!("{source}\n    rejected"));
            continue;
        }
        let json = parse_json(source);
        let got = (
            node_type(&json, COND),
            node_type(&json, CONSEQUENT),
            node_type(&json, ALTERNATE),
        );
        let want = (
            "ConditionalExpression".to_owned(),
            (*consequent).to_owned(),
            (*alternate).to_owned(),
        );
        if got != want {
            failures.push(format!(
                "{source}\n    expected: {want:?}\n    got:      {got:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} shapes read wrong:\n  {}",
        failures.len(),
        cases.len(),
        failures.join("\n  ")
    );
}

/// The annotation is dropped where no second `:` follows the arrow: the head is
/// the consequent (whatever expression it is) and the `type =>` after the `:` is
/// the alternate's arrow — tsc's `tryParse` rewind, and acorn's rejection.
#[test]
fn dropped_annotation_reads_the_head_as_the_consequent() {
    check_shapes(&[
        ("b ? (c) : d => e;", "Identifier", "ArrowFunctionExpression"),
        (
            "a ? b ? c : (d) : e => f;",
            "ConditionalExpression",
            "ArrowFunctionExpression",
        ),
        (
            "a ? (b + c) : d => e;",
            "BinaryExpression",
            "ArrowFunctionExpression",
        ),
        (
            "a ? (b = 1): c => d;",
            "AssignmentExpression",
            "ArrowFunctionExpression",
        ),
        (
            "a ? ([b]): c => d;",
            "ArrayExpression",
            "ArrowFunctionExpression",
        ),
        (
            "a ? ({ b }): c => d;",
            "ObjectExpression",
            "ArrowFunctionExpression",
        ),
        (
            "a ? (b, c): d => e;",
            "SequenceExpression",
            "ArrowFunctionExpression",
        ),
        // a generic head rewinds to a type assertion over the consequent
        (
            "a ? <T>(b): c => d;",
            "TSTypeAssertion",
            "ArrowFunctionExpression",
        ),
        // an async head rewinds to a call of `async`
        (
            "a ? async (b): c => d;",
            "CallExpression",
            "ArrowFunctionExpression",
        ),
    ]);
    // The head inside a consequent arrow's body: the arrow keeps no annotation and
    // its body is the bare head.
    let json = parse_json("x ? y => ({ y }) : z => ({ z });");
    assert!(!annotated(&json, CONSEQUENT));
    assert_eq!(
        node_type(&json, &format!("{CONSEQUENT}/body")),
        "ObjectExpression"
    );
    let json = parse_json("a ? (b) => (c): d => e;");
    assert!(!annotated(&json, CONSEQUENT));
    assert_eq!(
        node_type(&json, &format!("{CONSEQUENT}/body")),
        "Identifier"
    );
    // The corpus's nested shape: the outer annotation survives (a second `:` follows
    // its body), the inner one is dropped.
    let json = parse_json("a ? (b) : c => (d) : e => f;");
    assert!(annotated(&json, CONSEQUENT));
    assert_eq!(
        node_type(&json, &format!("{CONSEQUENT}/body")),
        "Identifier"
    );
    assert_eq!(node_type(&json, ALTERNATE), "ArrowFunctionExpression");
}

/// A second `:` after the arrow makes the annotation the only reading, so it is
/// kept — for every head kind that can carry one.
#[test]
fn kept_annotation_when_a_second_colon_follows() {
    for source in [
        "a ? (b): c => d : e;",
        "a ? (b: B): C => d : e;",
        "a ? <T>(b): c => d : e;",
        "a ? async (b): c => d : e;",
        "a ? (b): c => {} : d;",
        "a ? (b): c => d ? e : f : g;",
    ] {
        let json = parse_json(source);
        assert_eq!(node_type(&json, COND), "ConditionalExpression", "{source}");
        assert_eq!(
            node_type(&json, CONSEQUENT),
            "ArrowFunctionExpression",
            "{source}"
        );
        assert!(annotated(&json, CONSEQUENT), "{source}");
        assert_eq!(node_type(&json, ALTERNATE), "Identifier", "{source}");
    }
    // Nested: both annotations survive when each arrow is followed by a `:`.
    let json = parse_json("a ? (b): c => (d): e => f : g;");
    assert!(annotated(&json, CONSEQUENT));
    assert!(annotated(&json, &format!("{CONSEQUENT}/body")));
}

/// The bar reaches only the consequent's own grouping depth: inside a delimiter
/// opened since the `?`, in a function or class body and in a `yield` argument the
/// annotation is always allowed — where tsc passes `true` again.
#[test]
fn the_bar_is_lifted_inside_delimiters_bodies_and_yield() {
    let json = parse_json("a ? [(b): c => d] : e;");
    assert!(annotated(&json, &format!("{CONSEQUENT}/elements/0")));
    let json = parse_json("a ? ((b): c => d) : e;");
    assert!(annotated(&json, CONSEQUENT));
    let json = parse_json("a ? function () { return (b): c => d; } : e;");
    assert!(annotated(
        &json,
        &format!("{CONSEQUENT}/body/body/0/argument")
    ));
    let json = parse_json("a ? class { x = (b): c => d } : e;");
    assert!(annotated(&json, &format!("{CONSEQUENT}/body/body/0/value")));
    let json = parse_json("function* g() { a ? yield (b): c => d : e; }");
    assert!(annotated(
        &json,
        "/body/0/body/body/0/expression/consequent/argument"
    ));
}

/// The bar is inherited where tsc threads the flag: an arrow's concise body, the
/// alternate (from the *enclosing* rule) and an assignment's right side.
#[test]
fn the_bar_is_inherited_by_bodies_alternates_and_assignments() {
    let json = parse_json("x ? (y) => (z): w => v;");
    assert_eq!(
        node_type(&json, &format!("{CONSEQUENT}/body")),
        "Identifier"
    );
    assert_eq!(node_type(&json, ALTERNATE), "ArrowFunctionExpression");
    // An outer alternate is not barred, so its nested conditional re-bars only
    // its own consequent.
    let json = parse_json("a ? b : c ? (d): e => f;");
    assert_eq!(
        node_type(&json, &format!("{ALTERNATE}/consequent")),
        "Identifier"
    );
    assert_eq!(
        node_type(&json, &format!("{ALTERNATE}/alternate")),
        "ArrowFunctionExpression"
    );
    let json = parse_json("a ? b = (c): d => e;");
    assert_eq!(
        node_type(&json, &format!("{CONSEQUENT}/right")),
        "Identifier"
    );
    assert_eq!(node_type(&json, ALTERNATE), "ArrowFunctionExpression");
    // A call's arguments lift the bar, and the conditional inside re-bars.
    let json = parse_json("f(a ? (b): c => d);");
    assert_eq!(
        node_type(&json, "/body/0/expression/arguments/0/consequent"),
        "Identifier"
    );
}

/// A rewound parse drains the comments its abandoned half consumed again, exactly
/// once — and the abandoned body's regex and template re-lex cleanly.
#[test]
fn a_rewound_parse_re_drains_its_comments_once() {
    let source = "a ? (/* c1 */ b /* c2 */): c /* c3 */ => d;";
    assert_eq!(comment_count(source), 3);
    assert_eq!(
        format(source),
        "a ? /* c1 */ b /* c2 */ : (c) /* c3 */ => d;\n"
    );
    assert_eq!(
        format("a ? (b) : c => /:/.test(d);"),
        "a ? b : (c) => /:/.test(d);\n"
    );
    assert_eq!(
        format("a ? (b) : c => `${d}:${e}`;"),
        "a ? b : (c) => `${d}:${e}`;\n"
    );
}

/// A committed inner head consumes the `:` the enclosing speculation was waiting
/// for, so that speculation rewinds: `(d: D)` is unambiguous, keeps its annotation
/// and swallows `(d: D): E => e` whole, leaving nothing after the outer arrow. The
/// alternate then re-reads the same bytes under the lifted rule — tsc's outer
/// rewind, and the reason the two halves of the rule compose.
#[test]
fn a_committed_inner_head_rewinds_the_enclosing_speculation() {
    let json = parse_json("a ? (b) : c => (d: D): E => e;");
    assert_eq!(node_type(&json, CONSEQUENT), "Identifier");
    assert!(annotated(&json, &format!("{ALTERNATE}/body")));
    let json = parse_json("a ? (b) : c => (d: D): E => (f: F): G => g;");
    assert_eq!(node_type(&json, CONSEQUENT), "Identifier");
    assert!(annotated(&json, &format!("{ALTERNATE}/body")));
    assert!(annotated(&json, &format!("{ALTERNATE}/body/body")));
}

/// tsc COMMITS an *unambiguous* head to the annotation without asking — `(a: T)`,
/// `()`, `(...a)`, `(a?: T)`, and the same after `async`
/// (`isParenthesizedArrowFunctionExpressionWorker`'s `Tristate.True`, which reaches
/// `parseParenthesizedArrowFunctionExpression` with `allowReturnTypeInArrowFunction:
/// true`). That `true` reaches the arrow's BODY as well, so an annotated head inside
/// it keeps its own annotation and the conditional runs out of `:` — a syntax error
/// tsc raises and tsv must raise with it. Speculating on such a head instead reads a
/// truncated body, finds the `:` that the inner annotation would have taken, and
/// keeps the outer arrow: an over-acceptance no valid program can reach, but a
/// reading neither tsc nor acorn has.
///
/// The distinction is only about WHICH heads commit: an ambiguous head, a generic
/// head (`Tristate.Unknown` for tsc — a `<` never commits outside JSX) and an
/// unannotated head all stay speculative, and their bodies keep the bar.
#[test]
fn an_unambiguous_head_commits_and_lifts_the_bar_in_its_body() {
    // One per `Tristate.True` arm: `(a: T)`, `this`, `()`, `(...a)`, `(a?` with each
    // of its four followers, a parameter property — and the set behind `async`.
    for source in [
        "a ? (b: B): c => (d): e => f;",
        "a ? (this: T): c => (d): e => f;",
        "a ? (): c => (d): e => f;",
        "a ? (...b): c => (d): e => f;",
        "a ? ( /* c */ ...b): c => (d): e => f;",
        "a ? (b?: B): c => (d): e => f;",
        "a ? (b?): c => (d): e => f;",
        "a ? (b?, c): d => (e): f => g;",
        "a ? (b? = 1): c => (d): e => f;",
        "a ? (public b): c => (d): e => f;",
        "a ? (readonly b): c => (d): e => f;",
        "a ? async (b: B): c => (d): e => f;",
    ] {
        assert!(!accepts(source), "should reject: {source}");
    }
    // And one per arm tsc leaves `Unknown` or `False`, each of which HAS a
    // parenthesized reading, so committing would reject what tsc accepts. The `?`
    // followers are the near misses: `(a ? b : c)` and `(a ?? b)` are a conditional
    // and a coalesce, not an optional parameter; `(public as B)` is an assertion on
    // a name and `(readonly)` is just one.
    for source in [
        "a ? (b, c): d => (e): f => g;",
        "a ? (b = 1): c => (d): e => f;",
        "a ? ([b]): c => (d): e => f;",
        "a ? ({ b }): c => (d): e => f;",
        "a ? (b ? c : d): e => (f): g => h;",
        "a ? (b ?? c): d => (e): f => g;",
        "a ? (public as B): c => (d): e => f;",
        "a ? (readonly): c => (d): e => f;",
    ] {
        assert!(accepts(source), "should accept: {source}");
    }
    // The heads tsc leaves ambiguous keep the speculation, so the inner annotation
    // is refused and the outer one survives on the `:` it frees up.
    for source in [
        "a ? (b): c => (d): e => f;",
        "a ? <T>(b): c => (d): e => f;",
    ] {
        let json = parse_json(source);
        assert!(annotated(&json, CONSEQUENT), "{source}");
        assert_eq!(
            node_type(&json, &format!("{CONSEQUENT}/body")),
            "Identifier",
            "{source}"
        );
    }
    // A committed head is committed for its whole reading: the same shape with the
    // conditional's `:` present parses, and the body keeps ITS annotation.
    let json = parse_json("a ? (b: B): c => (d): e => f : g;");
    assert!(annotated(&json, CONSEQUENT));
    assert!(annotated(&json, &format!("{CONSEQUENT}/body")));
}

/// The rewound reading ends the statement where the body ends: a token on the
/// next line that cannot continue it is a new statement, so the label survives.
#[test]
fn asi_splits_a_label_off_a_rewound_consequent() {
    let json = parse_json("a ? (b): c => d\ne: f;");
    assert_eq!(node_type(&json, CONSEQUENT), "Identifier");
    assert_eq!(node_type(&json, "/body/1"), "LabeledStatement");
}

/// Rejections tsc shares: a committed unambiguous head or a lifted `yield`
/// argument leaves the conditional without its `:`; a head that is no parameter
/// list with no annotation is never in question; a `,` ends the body.
#[test]
fn rejections_tsc_shares() {
    for source in [
        "a ? (b: B): C => d;",
        "function* g() { a ? yield (b) : c => d; }",
        "a ? (b + c) => d : e;",
        "a ? (b): c => d, e : f;",
        "a ? (b + c): d => e : f;",
    ] {
        assert!(!accepts(source), "should reject: {source}");
    }
}
