// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The strict-mode-reserved words — `implements`, `interface`, `let`, `package`,
//! `private`, `protected`, `public`, `static`, `yield` — are ordinary names as far
//! as the *grammar* goes. §sec-identifiers-static-semantics-early-errors bars them
//! in **one bullet**, a Static Semantics early error, and tsv defers early errors
//! to the diagnostics layer, so it parses every one of them as a name.
//!
//! Nothing excludes them at the production level. `Identifier : IdentifierName but
//! not ReservedWord` doesn't reach them (none is a `ReservedWord`) except `yield`,
//! which `BindingIdentifier[Yield, Await] : Identifier | `yield` | `await``
//! readmits with **no** guard — the spec deliberately writes even the `[Yield]`
//! restriction as an early error there, and its own note says why (so ASI cannot
//! split `let ⏎ await 0;`). An `infer` type-parameter name is the same channel and
//! rides the same matrix.
//!
//! **Three channels, not one**, and the difference is whether the spec wrote the
//! bar as a production guard or as an early error:
//!
//! | channel | `yield` / `await` in a `[+Yield]` / `[+Await]` context |
//! | --- | --- |
//! | `BindingIdentifier` | admitted — no guard, early error DEFERRED |
//! | `IdentifierReference` | barred — `[~Yield]` / `[~Await]` guard in the production |
//! | `LabelIdentifier` | barred — identical production to `IdentifierReference` |
//!
//! So `function* g() { var yield = 1; }` and `async function h() { var await = 1; }`
//! parse, while `{ yield }`, `yield: ;`, `{ await }` and `await: ;` reject inside
//! those same functions. A guard is not a deferrable early error: in a `[+Yield]` /
//! `[+Await]` context the word is the **operator**, so the name reading is
//! unreachable rather than merely invalid. `await` carries a second, independent
//! bar — the **goal** bullet — which tsv enforces in every channel.
//!
//! Real tsc agrees: `ts.createSourceFile(...).parseDiagnostics` is empty for every
//! word in every position below, and prettier formats every one. Most of the list
//! was already accepted everywhere, purely because tsv's lexer never keyword-izes
//! those words — which is what made the holes tokenization artifacts rather than
//! rules. The holes had two shapes: `let`/`yield` were keyword-lexed, and
//! `implements`/`private`/`protected`/`public` had a competing syntactic role
//! (heritage clause, parameter accessibility modifier) that swallowed them. Both
//! now resolve the way tsc resolves them.
//!
//! This can't be a fixture: acorn-typescript enforces the early error and *rejects*
//! every case, so no `expected.json` oracle exists — the shapes are pinned here
//! against tsv itself, the way `keyword_type_reference.rs` pins the type-space
//! names acorn is over-strict about. The halves acorn *does* accept are fixtures
//! (`statements/labeled/contextual_keyword_name`,
//! `types/infer/contextual_keyword_name`, `types/interfaces/heritage_yield`).
//!
//! ⚠️ Several tests below assert a **node type**, not just an accept. That is
//! deliberate: an over-permissive parser can accept a widened word while building
//! the wrong node for it (a non-generator `yield` once produced a `YieldExpression`,
//! so `yield.foo` was a `MemberExpression` over one), and the drop-in wire contract
//! cares about that where an accept/reject assertion cannot see it.
//!
//! Contrast: `void` is a genuine `ReservedWord`, excluded at the *production*
//! level, so it keeps rejecting in every binding position (guards below).

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
}

fn accepts(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

/// `accepts` at `Goal::Script`, where `await` is not reserved by the goal bullet —
/// the only goal at which the `[Await]` binding cases are observable at all.
fn accepts_script(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse_with_goal(source, tsv_ts::Goal::Script, &arena).is_ok()
}

/// The `type` of the node at `pointer`, for asserting a widened word reaches the
/// wire as an `Identifier` rather than an operator node.
fn node_type_at(source: &str, pointer: &str) -> Option<String> {
    parse_json(source)
        .pointer(pointer)
        .and_then(|n| n.get("type"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// The full strict-mode-reserved list of
/// §sec-identifiers-static-semantics-early-errors, minus `interface` (which the
/// parser reads as a declaration starter, tested by its own fixtures). One
/// bullet bars all of them, so one verdict covers all of them: tsv parses each
/// as a name and defers.
///
/// Three of these have a *competing syntactic role* that used to block them in
/// specific positions — `implements` reads as the start of a heritage clause
/// after `class`, and `private`/`protected`/`public` read as parameter
/// accessibility modifiers. Both now resolve by lookahead, exactly as tsc does,
/// so the whole list behaves uniformly.
const STRICT_RESERVED: &[&str] = &[
    "let",
    "yield",
    "static",
    "package",
    "implements",
    "private",
    "protected",
    "public",
];

/// The subset acorn also rejects as a *name* everywhere, i.e. the words this
/// branch actually widened. The rest were already accepted because tsv's lexer
/// leaves them as plain `Identifier`s.
const WIDENED: &[&str] = &["let", "yield"];

/// Every `BindingIdentifier` position, as a template with `$` for the name.
/// Measured against real tsc `parseDiagnostics` (all accept) and prettier (all
/// format).
const BINDING_POSITIONS: &[&str] = &[
    "var $ = 1;",
    "let $ = 1;",
    "const $ = 1;",
    "function $() {}",
    "var v = function $() {};",
    "function* $() {}",
    "async function $() {}",
    "class $ {}",
    "var v = class $ {};",
    "function f($) {}",
    "function f($ = 1) {}",
    "var f = ($) => 1;",
    "var f = $ => 1;",
    "var {$} = o;",
    "function f(...$) {}",
    "try {} catch ($) {}",
    "for (var $ of a) {}",
    "for (var $ in a) {}",
    "import $ from 'm';",
    "import * as $ from 'm';",
    "import {a as $} from 'm';",
    "enum $ {}",
    "interface $ {}",
    "namespace $ {}",
    "type $ = 1;",
    "function f<$>() {}",
    "import $ = require('m');",
    "using $ = x;",
    "class C { constructor(readonly $: number) {} }",
    // the positions a competing syntactic role used to block
    "class $ extends B {}",
    "class $<T> {}",
    "export default class $ {}",
    "class C { m($) {} }",
    "class C { constructor($) {} }",
    "declare function f($): void;",
    "function f($: number) {}",
    "function f($, b) {}",
    // a `LabelIdentifier` and an `infer` type-parameter name are the same channel
    "$: for (;;) break $;",
    "type T = X extends Y ? infer $ : never;",
    // context-sensitive: a binding position inside a generator / async function.
    // Both bars there are early errors too (`BindingIdentifier[Yield, Await]`),
    // and tsc accepts.
    "function* g() { var $ = 1; }",
    "function* g($) {}",
    "function* g() { class $ {} }",
    "async function h() { var $ = 1; }",
    "async function h($) {}",
];

/// The one `(word, position)` cell where the competing role legitimately wins,
/// so the matrix must not demand an accept: after `class`, an `implements`
/// followed by an identifier-or-keyword is the heritage clause, which leaves
/// this declaration nameless. tsc only *recovers* on it (empty heritage list)
/// and prettier rejects it; asserted by
/// `class_implements_recovery_shapes_still_reject`.
const ROLE_COLLISION: &[(&str, &str)] = &[("implements", "class $ extends B {}")];

#[test]
fn strict_reserved_words_are_binding_names() {
    let mut failures = Vec::new();
    for word in STRICT_RESERVED {
        for tpl in BINDING_POSITIONS {
            if ROLE_COLLISION.contains(&(word, tpl)) {
                continue;
            }
            let src = tpl.replace('$', word);
            if !accepts(&src) {
                failures.push(src);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} binding positions rejected:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// The name reaches the wire as a plain `Identifier`, not a keyword node.
#[test]
fn widened_binding_name_is_an_identifier_node() {
    for word in WIDENED {
        let src = format!("var {word} = 1;");
        let json = parse_json(&src);
        let id = "/body/0/declarations/0/id";
        assert_eq!(
            json.pointer(&format!("{id}/type")).and_then(Value::as_str),
            Some("Identifier"),
            "`{src}` declarator id is an Identifier: {json}"
        );
        assert_eq!(
            json.pointer(&format!("{id}/name")).and_then(Value::as_str),
            Some(*word),
            "`{src}` declarator id carries the word: {json}"
        );
    }
}

/// The bug258 parser-widening hazard: a newly-admitted shape must survive
/// format → reparse → format unchanged, so the printer can't corrupt what the
/// parser just started accepting.
#[test]
fn widened_binding_names_are_format_idempotent() {
    let mut failures = Vec::new();
    for word in WIDENED {
        for tpl in BINDING_POSITIONS {
            let src = tpl.replace('$', word);
            if !accepts(&src) {
                continue; // covered by the acceptance test above
            }
            let once = format(&src);
            if !accepts(&once) {
                failures.push(format!("{src}\n    -> output does not reparse: {once}"));
                continue;
            }
            let twice = format(&once);
            if once != twice {
                failures.push(format!("{src}\n    -> {once:?} != {twice:?}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} non-idempotent shapes:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// The same hazard on the **reference** side, which the binding sweep above cannot
/// reach: a newly-admitted `IdentifierReference` must survive format → reparse →
/// format too. `let` earns the emphasis — statement-initial, its printed form is
/// re-read by the very `isLetDeclaration` lookahead that classified it, so a
/// printer that dropped or added a token here would flip the reading on the way
/// back in.
#[test]
fn widened_reference_forms_are_format_idempotent() {
    let mut failures = Vec::new();
    for src in [
        "let;",
        "let = 1;",
        "x = let;",
        "typeof let;",
        "let.x = 1;",
        "let++;",
        "let();",
        "new let();",
        "class C extends let {}",
        "do {} while (let);",
        "yield();",
        "yield++;",
        "new yield();",
        "class C extends yield {}",
        "x = yield;",
        "yield.foo;",
        "var [let] = a;",
        "function f([let]) {}",
        "try {} catch ([yield]) {}",
    ] {
        let once = format(src);
        if !accepts(&once) {
            failures.push(format!("{src}\n    -> output does not reparse: {once}"));
            continue;
        }
        let twice = format(&once);
        if once != twice {
            failures.push(format!("{src}\n    -> {once:?} != {twice:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} non-idempotent shapes:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// `let` is an `IdentifierReference` too. `Identifier : IdentifierName but not
/// ReservedWord` admits it — `let` is no `ReservedWord` — so its only bar in the
/// reference spelling is the same strict-mode early error tsv defers in the
/// binding spelling. What separates the two readings is a lookahead, not a rule:
/// statement-initial `let` heads a declaration exactly when a binding follows
/// (tsc's `isLetDeclaration`).
#[test]
fn let_is_a_reference_when_no_binding_follows() {
    for src in [
        "let;",
        "let = 1;",
        "x = let;",
        "typeof let;",
        "let.x = 1;",
        "let++;",
        "let();",
        "new let();",
        "class C extends let {}",
        "do {} while (let);",
        "function f(a = let) {}",
        "label: { let; }",
    ] {
        assert!(accepts(src), "`{src}` must parse");
    }
    // …and it reaches the wire as an `Identifier`, not some operator node.
    assert_eq!(
        node_type_at("let.x = 1;", "/body/0/expression/left/object").as_deref(),
        Some("Identifier"),
        "`let.x = 1` has an Identifier object"
    );
}

/// The one place the reference reading is barred, and it is barred by the
/// *grammar*: `ExpressionStatement` carries `[lookahead ∉ { …, `let` `[` }]`, so
/// `let [` can never begin one. `let[0] = 1` is therefore a declaration with an
/// invalid array binding pattern, not an indexed assignment — a syntax error, as
/// tsc reports (TS1181). A for-head commits to the declaration on the keyword
/// alone (tsc does the same), so it needs no lookahead restriction to agree.
#[test]
fn let_bracket_is_never_an_expression_statement() {
    for src in ["let[0] = 1;", "for (let[0] of a) {}", "for (let.x of a) {}"] {
        assert!(!accepts(src), "`{src}` must still reject");
    }
    // The declaration reading is untouched by the lookahead in every real form,
    // including across a line break (`let` carries no `[no LineTerminator here]`).
    for src in [
        "let x = 1;",
        "let [a] = b;",
        "let {a} = b;",
        "let x;",
        "let x, y;",
        "let let = 1;",
        "let yield = 1;",
        "let\nx = 1;",
    ] {
        assert!(accepts(src), "`{src}` must still be a declaration");
    }
    assert_eq!(
        node_type_at("let\nx = 1;", "/body/0").as_deref(),
        Some("VariableDeclaration"),
        "a line break does not split the declaration"
    );
}

/// Inside a generator `yield` is the operator, so the two *expression*-context
/// readers of the binding-name set must not take it: `yield =>` is not an arrow
/// and `{ yield }` is not a shorthand there. Real tsc rejects both (TS1005).
#[test]
fn yield_in_a_generator_is_still_the_operator() {
    for src in [
        "function* g() { var f = yield => 1; }",
        "function* g() { o = { yield }; }",
    ] {
        assert!(!accepts(src), "`{src}` must still reject");
    }
    // …and outside a generator both are ordinary identifier readings, which tsc
    // accepts.
    for src in ["var f = yield => 1;", "o = { yield };", "o = { let };"] {
        assert!(accepts(src), "`{src}` must parse");
    }
}

/// Binding-pattern **elements** take the widened words too, in all three pattern
/// contexts. tsv parses a binding pattern by running the **expression** parser and
/// converting (`parse_destructured_binding` runs `parse_array_expression` /
/// `parse_object_expression`, then `to_assignable`), so an element head asks the
/// `IdentifierReference` channel rather than the binding one — which is why these
/// went on rejecting for as long as that channel was narrow, and why widening it
/// closed a declaration, a parameter and a `catch` binding in one move. Object
/// shorthand always worked, having its own keyword arm.
#[test]
fn binding_pattern_elements_take_the_widened_words() {
    /// `(pattern-element templates, shorthand template)` per pattern context.
    const CONTEXTS: &[(&[&str], &str)] = &[
        (&["var [$] = a;", "var {a: $} = o;"], "var {$} = o;"),
        (
            &["function f([$]) {}", "function f({a: $}) {}"],
            "function f({$}) {}",
        ),
        (
            &["try {} catch ([$]) {}", "try {} catch ({a: $}) {}"],
            "try {} catch ({$}) {}",
        ),
    ];
    let mut failures = Vec::new();
    for word in WIDENED {
        for (element_forms, shorthand_form) in CONTEXTS {
            for tpl in element_forms.iter().chain(std::iter::once(shorthand_form)) {
                let src = tpl.replace('$', word);
                if !accepts(&src) {
                    failures.push(src);
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} binding-pattern element forms rejected:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
    // A `void` element still rejects — the production bar, not a deferred early
    // error, so the widening stayed a rule rather than a blanket loosening.
    assert!(!accepts("var [void] = a;"), "`var [void] = a` must reject");
}

/// Outside a generator `yield` is an `IdentifierReference` — `[~Yield] yield` is
/// right there in the production — so it must reach the wire as an **`Identifier`**.
///
/// The node type is the point, not the accept. An unconditional
/// `parse_yield_expression` accepted most of these too, but built a
/// `YieldExpression` for them: `yield.foo` came out as a `MemberExpression` over a
/// `YieldExpression`, a node the enclosing non-generator function cannot legally
/// contain. That is a wire-shape bug the drop-in contract cares about, which an
/// accept/reject assertion alone cannot catch — hence the pointer checks.
#[test]
fn yield_outside_a_generator_is_an_identifier_node() {
    for src in [
        "x = yield;",
        "yield.foo;",
        "f(yield);",
        "[yield];",
        "x = yield + 1;",
        "x = yield ? a : b;",
        "o = { yield };",
        "var f = yield => 1;",
        "yield();",
        "yield++;",
        "new yield();",
        "class C extends yield {}",
    ] {
        assert!(accepts(src), "`{src}` must parse");
    }
    for (src, pointer) in [
        ("x = yield;", "/body/0/expression/right"),
        ("yield.foo;", "/body/0/expression/object"),
        ("f(yield);", "/body/0/expression/arguments/0"),
        ("yield();", "/body/0/expression/callee"),
        ("yield++;", "/body/0/expression/argument"),
        ("new yield();", "/body/0/expression/callee"),
        ("class C extends yield {}", "/body/0/superClass"),
    ] {
        assert_eq!(
            node_type_at(src, pointer).as_deref(),
            Some("Identifier"),
            "`{src}` must yield an Identifier at {pointer}, not a YieldExpression"
        );
    }
    // Inside a generator the widened words are still legal *arguments* to the
    // operator — `YieldExpression : yield [no LT] AssignmentExpression`, and `let`
    // is an ordinary one. A keyword that can begin no expression at all is not.
    assert_eq!(
        node_type_at(
            "function* g() { yield let; }",
            "/body/0/body/body/0/expression/argument"
        )
        .as_deref(),
        Some("Identifier"),
        "`yield let` takes `let` as its argument"
    );
    for src in [
        "function* g() { yield var; }",
        "function* g() { yield const; }",
    ] {
        assert!(!accepts(src), "`{src}` must reject");
    }
    // …while inside a generator the very same spellings stay the operator.
    assert_eq!(
        node_type_at(
            "function* g() { x = yield; }",
            "/body/0/body/body/0/expression/right"
        )
        .as_deref(),
        Some("YieldExpression"),
        "inside a generator `yield` is still the operator"
    );
}

/// The disambiguation must not swallow the word's *real* role. Each of these
/// keeps reading as the modifier / heritage keyword, and the shape is asserted
/// so a lookahead that is merely permissive can't pass.
#[test]
fn the_competing_role_still_wins_where_it_should() {
    // `implements` before a type name is the heritage clause of an anonymous
    // class, not the class name (acorn rejects this; tsc and prettier accept).
    let json = parse_json("export default class implements I {}");
    let decl = "/body/0/declaration";
    assert_eq!(
        json.pointer(&format!("{decl}/id")),
        Some(&Value::Null),
        "anonymous class, `implements` is the heritage keyword: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{decl}/implements/0/expression/name"))
            .and_then(Value::as_str),
        Some("I"),
        "the heritage list holds `I`: {json}"
    );

    // `private` before a binding is a parameter property modifier, not the name.
    let json = parse_json("class C { constructor(private x) {} }");
    let param = "/body/0/body/body/0/value/params/0";
    assert_eq!(
        json.pointer(&format!("{param}/type"))
            .and_then(Value::as_str),
        Some("TSParameterProperty"),
        "`private x` is a parameter property: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{param}/accessibility"))
            .and_then(Value::as_str),
        Some("private"),
        "…carrying the accessibility modifier: {json}"
    );

    // …and with no binding after it, the same word IS the name.
    let json = parse_json("class C { constructor(private) {} }");
    assert_eq!(
        json.pointer(&format!("{param}/type"))
            .and_then(Value::as_str),
        Some("Identifier"),
        "`private` alone is the parameter name: {json}"
    );
}

/// tsc only *recovers* on these — it builds an empty heritage list — and
/// prettier rejects them outright ("A class declaration without the 'default'
/// modifier must have a name"). tsc's parser accepting a shape is not on its own
/// a reason to accept, so the lookahead deliberately leaves them rejected.
#[test]
fn class_implements_recovery_shapes_still_reject() {
    for src in [
        "class implements extends B {}",
        "class implements implements I {}",
    ] {
        assert!(!accepts(src), "`{src}` must still reject");
    }
}

/// A labelled item is a `Statement` or a `FunctionDeclaration`, never a lexical
/// declaration — so widening the label channel must not admit one.
#[test]
fn a_lexical_declaration_is_still_not_a_labelled_item() {
    for src in ["label: let x = 1;", "async: let x = 1;", "let: let x = 1;"] {
        assert!(!accepts(src), "`{src}` must still reject");
    }
}

/// A `LabelIdentifier` keeps its `[~Yield]` guard **in the production**
/// (`LabelIdentifier[Yield, Await] : Identifier | [~Yield] `yield` | [~Await]
/// `await``), unlike `BindingIdentifier`, whose same-looking bar is an early
/// error. So the label channel — declaration *and* reference — must reject
/// `yield` inside a generator even though the binding channel accepts it there.
/// test262 pins exactly this (`yield-as-label-identifier`, 25 files).
#[test]
fn yield_as_a_label_is_production_guarded_in_a_generator() {
    for src in [
        "function* g() { yield: ; }",
        "function* g() { yield: for (;;) break yield; }",
        "function* g() { for (;;) { continue yield; } }",
        "async function* g() { yield: ; }",
        "class C { *m() { yield: ; } }",
    ] {
        assert!(
            !accepts(src),
            "`{src}` must reject: `[~Yield]` is a production"
        );
    }
    // …while the same label outside a generator is only barred by the strict-mode
    // early error, which tsv defers — and the binding channel accepts `yield`
    // inside the generator regardless.
    for src in [
        "yield: 1;",
        "yield: for (;;) break yield;",
        "function* g() { var yield = 1; }",
    ] {
        assert!(accepts(src), "`{src}` must parse");
    }
}

/// `void` is a `ReservedWord`, excluded by the `Identifier` production rather
/// than by an early error — so it keeps rejecting in every binding position.
/// This is the row that makes the widening a rule and not a blanket loosening.
#[test]
fn reserved_words_are_still_not_binding_names() {
    for tpl in BINDING_POSITIONS {
        let src = tpl.replace('$', "void");
        assert!(!accepts(&src), "`{src}` must still reject");
    }
}

/// `await` has **two** independent bars, and tsv answers them differently because
/// the spec writes them differently. Both live in
/// §sec-identifiers-static-semantics-early-errors:
///
/// ```text
/// BindingIdentifier : `await`   — Syntax Error if the goal symbol is Module.
/// BindingIdentifier[Yield, Await] : `await`
///                               — Syntax Error if this production has an [Await] parameter.
/// ```
///
/// The **goal** bullet is enforced (it is what makes `Goal::Script` observable at
/// all). The **`[Await]`** bullet is deferred, exactly like its `[Yield]` twin —
/// `BindingIdentifier` carries no guard for either word, tsc's parser defers both
/// (TS1359 is a checker diagnostic, same bucket as TS1212 for `let`), and prettier
/// formats both. So `async function h() { var await = 1; }` parses at Script goal.
#[test]
fn await_as_a_binding_name_splits_goal_from_await_context() {
    // The goal bullet: reserved as a name under `Goal::Module`, wherever it sits.
    for src in [
        "var await = 1;",
        "function f(await) {}",
        "import type await from 'm';",
        "class C { constructor(readonly await: number) {} }",
        "async function h() { var await = 1; }",
    ] {
        assert!(!accepts(src), "`{src}` must reject at Module goal");
    }
    // The `[Await]` bullet: deferred, so a `[+Await]` context is no bar at Script.
    for src in [
        "var await = 1;",
        "async function h() { var await = 1; }",
        "async function h(await) {}",
        "async function h() { class await {} }",
        "async function h() { function await() {} }",
    ] {
        assert!(accepts_script(src), "`{src}` must parse at Script goal");
    }
}

/// …while an `IdentifierReference` / `LabelIdentifier` `await` keeps the `[~Await]`
/// guard its production carries, so a `[+Await]` context still bars it even at
/// Script goal. Same shape as the `yield` label test above — one predicate serves
/// both words because the two productions are character-for-character identical.
/// tsc agrees (TS1109 on every case below).
#[test]
fn await_as_a_reference_is_production_guarded_in_an_async_function() {
    for src in [
        "async function h() { await: ; }",
        "async function h() { for (;;) break await; }",
        "async function h() { o = { await }; }",
        "async function h() { var f = await => 1; }",
        "async function h() { interface A extends await {} }",
    ] {
        assert!(
            !accepts_script(src),
            "`{src}` must reject: `[~Await]` is a production guard"
        );
    }
    // The same spellings are fine outside the `[+Await]` context.
    for src in ["await: ;", "o = { await };", "interface A extends await {}"] {
        assert!(accepts_script(src), "`{src}` must parse at Script goal");
    }
}

/// The heritage head is an `IdentifierReference`, so it takes the guards rather
/// than the deferral — the one place the binding/reference split is observable for
/// `yield` in a type position. tsc lands identically by parsing heritage with its
/// *expression* parser (TS1109), while a plain type annotation, which reaches no
/// expression parser in either implementation, is unaffected.
#[test]
fn a_heritage_head_is_a_reference_not_a_binding() {
    for src in [
        "function* g() { interface A extends yield {} }",
        "function* g() { class B implements yield {} }",
    ] {
        assert!(!accepts(src), "`{src}` must reject inside a generator");
    }
    for src in [
        "interface A extends yield {}",
        "class B implements yield {}",
        "function* g() { let x: yield; }",
        "function* g() { type T = yield.Foo; }",
    ] {
        assert!(accepts(src), "`{src}` must parse");
    }
}
