// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A type-argument list is TRIED past its first separator, and where the tokens there
//! spell no list the `<` is the comparison operator
//! (`Parser::parse_type_arguments_or_rewind`, `docs/conformance_svelte.md` §TypeScript
//! Corrections).
//!
//! The lookahead that admits a list grades its first argument and matches delimiters
//! behind its first `,`, so `<b, c + d>(e)` passes it — the tail of two comparisons in
//! comma siblings, which tsc and acorn-typescript both read as comparisons. The
//! oracle-backed body of the rule is fixtures
//! (`expressions/binary/relational_sibling_value_tail`, its Svelte twin under
//! `svelte/expressions`, and the refused side in
//! `relational_sibling_value_tail_refused_svelte_divergence`). What is pinned here is
//! what a fixture cannot hold:
//!
//! - **spellings that are not fixed points** — a redundant pair the printer strips, a
//!   `new` without its argument list;
//! - **rejections line by line** — a `tsv_rejects.txt` fixture verifies its first error
//!   and no more;
//! - **which error is reported** when the list and the operator reading both fail — an
//!   `input_invalid_*` file asserts only that both parsers reject;
//! - **the edges of the rule**: what stays a list, what stays the list's own error, and
//!   how the work is bounded where one tried list stands inside another.

use serde_json::Value;
use std::time::{Duration, Instant};

/// `source` formatted, asserted a fixed point.
fn stable(source: &str) -> String {
    let output = tsv_ts::format_str(source).expect("the case parses");
    assert_eq!(
        tsv_ts::format_str(&output).expect("the output reparses"),
        output,
        "not a fixed point: {source}"
    );
    output.trim_end().to_string()
}

/// Every node kind in `source`'s wire, pre-order.
fn node_kinds(source: &str) -> Vec<String> {
    fn walk(node: &Value, out: &mut Vec<String>) {
        match node {
            Value::Object(map) => {
                if let Some(kind) = map.get("type").and_then(Value::as_str) {
                    out.push(kind.to_owned());
                }
                map.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("the case parses");
    let json = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    let mut out = Vec::new();
    walk(&json, &mut out);
    out
}

/// How many type-argument lists `source` holds.
fn type_argument_lists(source: &str) -> usize {
    node_kinds(source)
        .iter()
        .filter(|kind| *kind == "TSTypeParameterInstantiation")
        .count()
}

/// Whether `source` holds a type-argument list anywhere.
fn has_type_arguments(source: &str) -> bool {
    type_argument_lists(source) > 0
}

/// How many binary expressions `source` holds.
fn binary_count(source: &str) -> usize {
    node_kinds(source)
        .iter()
        .filter(|kind| *kind == "BinaryExpression")
        .count()
}

/// `source` prints as `printed`, a fixed point, and reads as comparisons — at least the
/// `<` and the `>` — with no type-argument list, in the source and in the output alike.
#[track_caller]
fn assert_two_comparisons(source: &str, printed: &str) {
    assert_eq!(stable(source), printed, "printed from {source:?}");
    for text in [source, printed] {
        assert!(!has_type_arguments(text), "a list in {text:?}");
        assert!(binary_count(text) >= 2, "two comparisons in {text:?}");
    }
}

fn rejects(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_err()
}

/// The rendered error for `source`: its message line and its `line:col` header.
fn error(source: &str) -> (String, String) {
    let arena = bumpalo::Bump::new();
    let rendered = tsv_ts::parse(source, &arena)
        .expect_err("the case must not parse")
        .to_string();
    let mut lines = rendered.lines();
    let message = lines.next().expect("a message line").to_owned();
    let header = lines
        .next()
        .and_then(|located| located.split(' ').next())
        .expect("a located `line:col` line")
        .to_owned();
    (message, header)
}

#[test]
fn a_tail_that_is_no_list_reads_as_two_comparisons() {
    for (source, printed) in [
        // A key's `:` between the siblings.
        (
            "x = { p: a < b, q: c > (d - e) * 2 };",
            "x = { p: a < b, q: c > (d - e) * 2 };",
        ),
        (
            "x = { p: a < b, 'q-r': c > (d, e) };",
            "x = { p: a < b, 'q-r': c > (d, e) };",
        ),
        (
            "x = { p: a < b, [q]: c > `t` };",
            "x = { p: a < b, [q]: c > `t` };",
        ),
        (
            "x = { p: a < b, ...c > (d, e) };",
            "x = { p: a < b, ...(c > (d, e)) };",
        ),
        // An `=`: declarators, defaults, enum members.
        (
            "const p = a < b, q = c > (d, e);",
            "const p = a < b,\n\tq = c > (d, e);",
        ),
        (
            "let p: boolean = a < b, q: boolean = c > (d, e);",
            "let p: boolean = a < b,\n\tq: boolean = c > (d, e);",
        ),
        (
            "function fn(p = a < b, q = c > (d, e)) {}",
            "function fn(p = a < b, q = c > (d, e)) {}",
        ),
        (
            "const { p = a < b, q = c > (d, e) } = obj;",
            "const { p = a < b, q = c > (d, e) } = obj;",
        ),
        (
            "const [p = a < b, q = c > (d, e)] = arr;",
            "const [p = a < b, q = c > (d, e)] = arr;",
        ),
        (
            "enum E { A = a < b, B = c > (d, e) }",
            "enum E {\n\tA = a < b,\n\tB = c > (d, e)\n}",
        ),
        (
            "for (let i = 0, p = a < b, q = c > (d, e); ; ) {}",
            "for (let i = 0, p = a < b, q = c > (d, e); ;) {}",
        ),
        // A comma alone, and a token no type takes behind it.
        ("fn(a < b, c + d > (e, f));", "fn(a < b, c + d > (e, f));"),
        ("fn(a < b, c() > (d, e));", "fn(a < b, c() > (d, e));"),
        ("fn(a < b, c.d() > (e, f));", "fn(a < b, c.d() > (e, f));"),
        ("fn(a < b, c?.d > (e, f));", "fn(a < b, c?.d > (e, f));"),
        (
            "fn(a < b, c ? d : e > (f, g));",
            "fn(a < b, c ? d : e > (f, g));",
        ),
        (
            "fn(a < b, c as T > (d, e));",
            "fn(a < b, (c as T) > (d, e));",
        ),
        (
            "fn(a < b, await c > (d, e));",
            "fn(a < b, (await c) > (d, e));",
        ),
        ("fn(a < b, -c > (d, e));", "fn(a < b, -c > (d, e));"),
        ("fn(a < b, c in d > (e, f));", "fn(a < b, c in d > (e, f));"),
        ("fn(a < b, c && d > (e, f));", "fn(a < b, c && d > (e, f));"),
        ("fn(a < b, c = d > (e, f));", "fn(a < b, (c = d > (e, f)));"),
        ("fn(a < b, c + d > `t`);", "fn(a < b, c + d > `t`);"),
        (
            "x = [a < b, c * d > (e, f)];",
            "x = [a < b, c * d > (e, f)];",
        ),
        (
            "new Cls(a < b, c + d > (e, f));",
            "new Cls(a < b, c + d > (e, f));",
        ),
        ("a < b, c + d > (e, f);", "(a < b, c + d > (e, f));"),
        (
            "x = `${(a < b, c + d > (e, f))}`;",
            "x = `${(a < b, c + d > (e, f))}`;",
        ),
        // A sibling may stand between the two, and the `<` side may hold any type-shaped
        // operand.
        (
            "fn(a < b, c, d + e > (f, g));",
            "fn(a < b, c, d + e > (f, g));",
        ),
        (
            "fn(a.b < c, d + e > (f, g));",
            "fn(a.b < c, d + e > (f, g));",
        ),
        ("fn(a < 1, b + c > (d, e));", "fn(a < 1, b + c > (d, e));"),
        (
            "fn(a < 's', b + c > (d, e));",
            "fn(a < 's', b + c > (d, e));",
        ),
        (
            "fn(a < typeof b, c + d > (e, f));",
            "fn(a < typeof b, c + d > (e, f));",
        ),
        // A follower pair the printer strips: the output holds no `(` behind the `>`.
        (
            "const p = a < b, q = c > (d + 1);",
            "const p = a < b,\n\tq = c > d + 1;",
        ),
        ("fn(a < b, c + d > (e));", "fn(a < b, c + d > e);"),
        // An arrow function behind a token the list stopped at first.
        (
            "x = { p: a < b, q: c => c > (d - e) * 2 };",
            "x = { p: a < b, q: (c) => c > (d - e) * 2 };",
        ),
        (
            "fn(a < b, c + d > (e, f), g => g > (h, i));",
            "fn(a < b, c + d > (e, f), (g) => g > (h, i));",
        ),
        (
            "fn(a < b, async c => c > (d, e));",
            "fn(a < b, async (c) => c > (d, e));",
        ),
        // A paren pair ahead of a token no type continues with: one the printer keeps,
        // and a shell it strips, where the name left bare takes that token no better
        // than the parenthesized type did.
        (
            "check(lo < hi, (flags & MASK) !== 0 && n > (lo + hi) / 2);",
            "check(lo < hi, (flags & MASK) !== 0 && n > (lo + hi) / 2);",
        ),
        (
            "fn(a < b, (c | 0) === c, d > (e, f));",
            "fn(a < b, (c | 0) === c, d > (e, f));",
        ),
        ("fn(a < b, (c) + d > (e, f));", "fn(a < b, c + d > (e, f));"),
        ("fn(a < b, (c)(d) > (e, f));", "fn(a < b, c(d) > (e, f));"),
        // The list stops AT a `[` here — no type opens on a `-` ahead of a bracket —
        // with no line break ahead of it, so there is nothing for the printer to fold.
        ("fn(a < b, -[c] > (d, e));", "fn(a < b, -[c] > (d, e));"),
    ] {
        assert_two_comparisons(source, printed);
    }
}

#[test]
fn both_positions_with_an_operator_reading_fall_back() {
    for (source, printed) in [
        // The subscript loop, behind a call and behind a member.
        (
            "fn(g() < a, b + c > (d, e));",
            "fn(g() < a, b + c > (d, e));",
        ),
        // (a tagged template's tag is the same loop)
        ("fn(tag < a, b + c > `t`);", "fn(tag < a, b + c > `t`);"),
        // The `new` callee: an argument-less `new` compared with what follows.
        (
            "fn(new Cls < a, b + c > (d, e));",
            "fn(new Cls() < a, b + c > (d, e));",
        ),
        (
            "fn(new a.b < c, d ? e : f > (g, h));",
            "fn(new a.b() < c, d ? e : f > (g, h));",
        ),
        // A class heritage takes a list only where a call follows it, through the same
        // loop.
        (
            "class K extends fn(a < b, c + d > (e, f)) {}",
            "class K extends fn(a < b, c + d > (e, f)) {}",
        ),
    ] {
        assert_two_comparisons(source, printed);
    }
    // Behind `?.` and in a decorator a `<` has no operator reading, so the list is read
    // committed and its tail's error is the line's.
    for source in ["fn(a?.<b, c + d>(e));", "@dec<b, c + d>(e) class K {}"] {
        assert!(rejects(source), "{source:?}");
    }
}

#[test]
fn a_tail_that_is_a_list_stays_one() {
    for (source, printed) in [
        ("fn(a < b, c > (d, e));", "fn(a<b, c>(d, e));"),
        ("fn(a < b, c.d > (e, f));", "fn(a<b, c.d>(e, f));"),
        ("fn(a < b, () => c > (d, e));", "fn(a<b, () => c>(d, e));"),
        ("fn(a < b, (c) => c > (d, e));", "fn(a<b, (c) => c>(d, e));"),
        (
            "fn(a < b, [c], { d } > (e, f));",
            "fn(a<b, [c], { d }>(e, f));",
        ),
        ("x = f<A, B,>(c);", "x = f<A, B>(c);"),
    ] {
        assert_eq!(stable(source), printed, "printed from {source:?}");
        assert!(has_type_arguments(source), "{source:?}");
        assert!(has_type_arguments(printed), "{printed:?}");
    }
}

#[test]
fn a_failure_in_the_first_argument_stays_the_lists_own() {
    // The lookahead graded the first argument and admitted the list — for some of
    // these on purpose, to keep a chain with no sound printed form rejected. A
    // separator further on changes nothing: the list never got that far.
    for source in [
        "x = f<(A)<C>>(c);",
        "x = g(f<(A)<C>, D>(c));",
        "x = g(f<-1<C>(d), D>(c));",
        "x = a < { ...s } > (c, d);",
        "x = g(a < { ...s }, b > (c, d));",
        "x = a < (b = c) > (d, e);",
        "x = g(a < (b = c), d > (e, f));",
    ] {
        assert!(rejects(source), "{source:?}");
    }
}

#[test]
fn a_tail_the_printer_would_complete_stays_rejected() {
    for source in [
        // The list stopped at a `=>`: the bare parameter prints in a paren pair, a
        // function type's.
        "fn(a < b, c => c > (d - e) * 2);",
        "fn(a < b, c => d, e > (f, g));",
        "fn(a < b, [c => d], e > (f, g));",
        "fn(a < b, { m: c => d }, e > (f, g));",
        "fn(a < b, (c => d), e > (f, g));",
        "fn(a < b, c => d => d > (e, f));",
        "fn(a < b, c /* k */ => c > (d, e));",
        "x = [a < b, c => c > (d, e)];",
        // (wider than the hazard: no pair makes `c + 1` a function type's return)
        "fn(a < b, c => c + 1 > (d, e));",
        // The list stopped at a `.` or a `<` right behind a `)`: the shell is stripped,
        // and the name it leaves takes the member or the list behind it.
        "fn(a < b, (c).d > (e, f));",
        "fn(a < b, ((c)).d > (e, f));",
        "fn(a < b, (c)\n\t.d > (e, f));",
        "fn(a < b, (c) < d >> (e, f));",
        // (wider than the hazard: the printer keeps this pair)
        "fn(a < b, (c | d).e > (f, g));",
        // The list stopped at a `[` or a `<` past a line break, which the printer folds.
        "fn(a < b, c\n\t[d] > (e, f));",
        "fn(a < b, c\n\t<d>, e > (f, g));",
        "fn(a < b, c\n\t< d >> (e, f));",
        "fn(a < b, typeof c\n\t<d>, e > (f, g));",
    ] {
        assert!(rejects(source), "{source:?}");
    }
    // The fixture's first line, with the message its `tsv_rejects.txt` pins.
    assert_eq!(
        error("fn(a < b, c => c > (d - e) * 2);"),
        ("Expected '>', found ''=>''".to_owned(), "1:13".to_owned())
    );
}

#[test]
fn the_same_comparisons_parse_once_no_list_can_be_read() {
    // The authoring the refused spellings can take: the first comparison written from
    // the other side, either one hoisted, or a `function` in the arrow's place.
    for (source, printed) in [
        (
            "fn(b > a, c => c > (d - e) * 2);",
            "fn(b > a, (c) => c > (d - e) * 2);",
        ),
        (
            "const lt = a < b;\nfn(lt, c => c > (d - e) * 2);",
            "const lt = a < b;\nfn(lt, (c) => c > (d - e) * 2);",
        ),
        (
            "fn(a < b, function (c) {\n\treturn c > (d - e) * 2;\n});",
            "fn(a < b, function (c) {\n\treturn c > (d - e) * 2;\n});",
        ),
        (
            "const gt = c.d > (e, f);\nfn(a < b, gt);",
            "const gt = c.d > (e, f);\nfn(a < b, gt);",
        ),
    ] {
        assert_eq!(stable(source), printed, "printed from {source:?}");
        assert!(!has_type_arguments(source), "{source:?}");
    }
}

#[test]
fn a_shell_behind_a_prefix_operator_is_not_refused() {
    // The known gap of the refusal: the list stops AT the shell's `(` here, which reads
    // as a call's from where the parser stands. The comparisons parse, the shell is
    // stripped, and the output is a generic call to every parser — so neither line is a
    // fixed point. Refusing them is the fix this pins the absence of.
    for (source, printed, reread) in [
        (
            "fn(a < b, typeof (c) > (d, e));",
            "fn(a < b, typeof c > (d, e));\n",
            "fn(a<b, typeof c>(d, e));\n",
        ),
        (
            "fn(a < b, -(1) > (d, e));",
            "fn(a < b, -1 > (d, e));\n",
            "fn(a<b, -1>(d, e));\n",
        ),
    ] {
        let output = tsv_ts::format_str(source).expect("the case parses");
        assert_eq!(output, printed, "printed from {source:?}");
        assert_eq!(
            tsv_ts::format_str(&output).expect("the output reparses"),
            reread,
            "the second pass of {source:?}"
        );
    }
}

#[test]
fn a_pair_the_printer_adds_is_not_read_by_the_refusal() {
    // The refusal reads the source's tokens; these pairs are the printer's own.
    //
    // A number's member access prints in a pair, which is then a shell ahead of a `.`:
    // the comparisons parse, and their output is rejected on a second pass.
    let output = tsv_ts::format_str("fn(a < b, 5..toFixed() > (c, d));").expect("the case parses");
    assert_eq!(output, "fn(a < b, (5).toFixed() > (c, d));\n");
    assert!(rejects(&output), "{output:?}");
    // An assignment and an arrow function's conditional body print in a pair too. Both
    // lines stay two comparisons to this parser and to acorn-typescript, and fixed
    // points; the compiler's recovering list parse claims the printed pair and rejects
    // the line.
    for (source, printed) in [
        (
            "fn(a < b, c = d, e > (f, g));",
            "fn(a < b, (c = d), e > (f, g));",
        ),
        (
            "fn(a < b, (c) => c ? d : e, f > (g, h));",
            "fn(a < b, (c) => (c ? d : e), f > (g, h));",
        ),
    ] {
        assert_two_comparisons(source, printed);
    }
}

#[test]
fn a_type_only_tsc_reads_past_the_separator_is_no_list() {
    // A non-null `!`, a parameter default and an import that is no import type are types
    // to tsc's parser alone, which its checker then refuses as syntax; acorn-typescript
    // reads comparisons, and so does this. Behind a follower pair that stays, each
    // parser reads the output as it read the input.
    for (source, printed) in [
        ("fn(a < b, c! > (d, e));", "fn(a < b, c! > (d, e));"),
        ("fn(a < b, !c > (d, e));", "fn(a < b, !c > (d, e));"),
        (
            "fn(a < b, (c = 1) => c > (d, e));",
            "fn(a < b, (c = 1) => c > (d, e));",
        ),
        (
            "fn(a < b, import(c) > (d, e));",
            "fn(a < b, import(c) > (d, e));",
        ),
        // A redundant follower pair is stripped like any other.
        ("fn(a < b, c! > (d));", "fn(a < b, c! > d);"),
    ] {
        assert_two_comparisons(source, printed);
    }
}

#[test]
fn a_type_only_acorn_reads_past_the_separator_is_no_list() {
    // The split the other way round: a negative number behind a postfix is a literal
    // type to acorn-typescript alone, which reads a generic call; tsc reads comparisons,
    // and so does this.
    for (source, printed) in [
        ("fn(a < b, -1(c) > (d, e));", "fn(a < b, -1(c) > (d, e));"),
        ("fn(a < b, -5..x > (d, e));", "fn(a < b, -(5).x > (d, e));"),
    ] {
        assert_two_comparisons(source, printed);
    }
}

#[test]
fn when_both_readings_fail_the_error_that_reached_further_is_reported() {
    for (source, message, header) in [
        // The operator reading got further: its error is about the text as written.
        (
            "fn(a < b, c + d > (e);",
            "Expected ',' or ')' after list element, found ';'",
            "1:22",
        ),
        (
            "x = { p: a < b, q: c > (d };",
            "Expected ')', found '}'",
            "1:27",
        ),
        (
            "fn(a < b, c + > (d));",
            "Expected expression, found '>'",
            "1:15",
        ),
        // Both stopped at one token: the list's error, the first reading tried.
        (
            "fn(a < b, c : d > (e));",
            "Expected '>', found '':''",
            "1:13",
        ),
        // A refused fallback reports the list's error where the list stopped.
        (
            "fn(a < b, (c).d > (e, f));",
            "Expected '>', found ''.''",
            "1:14",
        ),
    ] {
        assert_eq!(
            error(source),
            (message.to_owned(), header.to_owned()),
            "{source:?}"
        );
    }
}

#[test]
fn an_embedded_parse_reports_the_further_error_too() {
    // An expression island's parser is built by another constructor than a document's.
    let arena = bumpalo::Bump::new();
    let source = "fn(a < b, c + d > (e)";
    let error = tsv_ts::parse_expression_with_comments(source, 100, &arena)
        .expect_err("the island must not parse");
    // past the `)`, in the host's coordinates — not at the `+` the list stopped on
    assert_eq!(error.position(), Some(100 + source.len()));
}

#[test]
fn a_goal_gate_in_the_tail_is_reported_as_it_stands() {
    // At the script goal `import.meta` is a goal gate, and the format fallback reads the
    // gate's mark off the error the parse ends on. The tail that meets one is not
    // abandoned: here the operator reading fails at the type literal's `;`, short of the
    // `import.meta`, and the rewind of the speculation around the list — an arrow head,
    // another tried list — would drop the gate's error with the list.
    for source in [
        "c ? (x = f<A, { p: X; [import.meta]: 1 }>(y)) : T => z;",
        "fn(a < b, { [k(f<A, { p: X; [import.meta]: 1 }>(y))]: 1 }, c + d > (e));",
    ] {
        let arena = bumpalo::Bump::new();
        let error = tsv_ts::parse_with_goal(source, tsv_ts::Goal::Script, &arena)
            .expect_err("`import.meta` is no script's");
        assert!(error.is_goal_gated(), "{source:?}: {error}");
    }
}

#[test]
fn a_remembered_failure_is_not_reused_in_another_context() {
    // Each line holds a list that fails past its separator in the context it is first
    // read in and parses in the one it is read in again, so its `<` has to be tried
    // again rather than remembered as a failure. Every one reads as a generic call.
    for (source, printed) in [
        // `[Await]`: the parenthesized head is tried as an arrow function's, whose
        // parameter default is read under the arrow's own context, where `await k` is
        // no expression. The head is then the grouped expression it is, read under the
        // enclosing async function.
        (
            "async function g() {\n\treturn c ? (x = f<A, { [await k]: 1 }>(y)) : T => z;\n}",
            "async function g() {\n\treturn c ? (x = f<A, { [await k]: 1 }>(y)) : (T) => z;\n}",
        ),
        // `[Yield]`, the other way round: a function type's parameter annotation is
        // read under the enclosing generator, where `1 + yield` is no expression, and
        // read again as an arrow function's under the arrow's own context.
        (
            "function* g() {\n\tfn(a < b, (p: { [k(h<A, { [1 + yield]: 1 }>(y))]: 1 }) => p + 1 > (z));\n}",
            "function* g() {\n\tfn(a < b, (p: { [k(h<A, { [1 + yield]: 1 }>(y))]: 1 }) => p + 1 > z);\n}",
        ),
    ] {
        assert_eq!(stable(source), printed, "printed from {source:?}");
        assert_eq!(type_argument_lists(source), 1, "{source:?}");
        assert_eq!(type_argument_lists(printed), 1, "{printed:?}");
    }
}

#[test]
fn an_abandoned_list_answers_for_its_own_less_than_alone() {
    // A `<` further on is another question: a later sibling of the same call, a later
    // statement.
    for source in [
        "fn(a < b, c + d > (e, f), g<A, B>(h));",
        "fn(a < b, c + d > (e, f));\nx = g<A, B>(h);",
    ] {
        assert_eq!(stable(source), source);
        assert_eq!(type_argument_lists(source), 1, "{source:?}");
        // `a < b`, `c + d` and the `>` over it
        assert_eq!(binary_count(source), 3, "{source:?}");
    }
}

#[test]
fn a_list_tried_inside_a_tried_list_is_tried_once() {
    // Each level's computed key is read by the list tried around it and again by the
    // operator reading that replaces it, so a list tried inside the key would be tried
    // twice per level around it — the work doubling at every level — were its failure
    // not remembered. Deep enough that the doubled work takes minutes; remembered, the
    // parse is a few passes over a short line.
    let mut source = "k".to_owned();
    for _ in 0..22 {
        source = format!("a < b, {{ [({source})]: 1 }}, c + d > (e)");
    }
    let source = format!("fn({source});");
    let started = Instant::now();
    let arena = bumpalo::Bump::new();
    assert!(tsv_ts::parse(&source, &arena).is_ok());
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "nested tried lists took {:?}",
        started.elapsed()
    );
}
