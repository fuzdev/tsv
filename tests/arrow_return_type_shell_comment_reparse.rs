// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A comment written last inside a redundant pair down the right edge of an arrow's return
//! type, where a line break or a member pair before the `=>` would end the arrow: the output
//! must parse, and must read as the program the input was.
//!
//! The `=>` is `[no LineTerminator here]` (ECMA-262 `ArrowFunction`), so a multi-line block
//! printed right before it ends the expression, and a `//` there needs a pair to hold its
//! line break. The fixtures `typescript/expressions/arrow/return_type_nested_shell_comment_prettier_divergence`
//! and `typescript/expressions/arrow/return_type_param_shaped_shell_comment_prettier_divergence` pin
//! the printed forms for a sample of these shapes. No fixture can carry the claim that the
//! output parses for every shape: a fixture's variants are graded against its input, and
//! prettier's own output from these spellings does not parse. So each shape is formatted here,
//! at a script position and in a Svelte `<script>` and template expression, and the output is
//! reparsed and compared with the input through tsv's wire, with the type grouping pairs
//! (which the print adds and removes by design) unwrapped and positions dropped; the root's
//! comment list stays, so a dropped or duplicated comment fails as well.

use serde_json::Value;

/// Return types holding `{K}` in a shell down their right edge. The first group has a paren
/// form (the outer pair can stay before `=>`). The second is a doubled pair around one type:
/// most have none (a lone pair around a name, a keyword type, `this`, a type literal or a
/// tuple reads as a function type's parameter list, so the type keeps a pair of its own
/// inside the outer one), and a qualified name, a generic, an array, a literal and a type
/// operator, which do, are its controls (a `readonly` tuple has none: tsc skips the modifier
/// and reads the tuple as a parameter). Every shape is graded with its comments in source
/// order. The grade is tsv's own reparse, which accepts some outputs tsc rejects (a lone pair
/// around a `readonly` tuple before `=>` among them), so tsc's reading of these shapes is
/// graded outside this suite.
const SHAPES: &[&str] = &[
    "(B | (C{K}))",
    "(A | B | (C{K}))",
    "(B | ((C{K})))",
    "(| B | (C{K}))",
    "(B & (C{K}))",
    "(keyof (C{K}))",
    "(readonly (C{K})[])",
    "(A extends B ? C : (D{K}))",
    "(A extends B ? C : X extends Y ? Z : (D{K}))",
    "(B | C & (D{K}))",
    "(B | (C[]{K}))",
    "(B | (G<T>{K}))",
    "(B | (keyof C{K}))",
    "(B | (readonly C[]{K}))",
    "(B | (readonly [C]{K}))",
    "(new () => (C{K}))",
    "(new () => B | (C{K}))",
    "(B | (A.C{K}))",
    "(B | ('s'{K}))",
    "(B | ({ a: C }{K}))",
    "(B | ([C]{K}))",
    "(B | (string{K}))",
    "(B | (C{K}) /* y */)",
    "(/* l */ B | (C{K}))",
    "(B | ((p: P) => C{K}))",
    "(B & (C | D{K}))",
    "(B | (A extends B ? C : D{K}))",
    "(B | (C | (D{K})))",
    "((B | (C{K})))",
    "(B | (C{K}) /* z\n */)",
    "(B | (C /* q */{K}))",
    "(B | (// y\nC{K}))",
    "((G<{ a: C /* i */ }>{K}))",
    // a directive ahead of the shell freezes it, comments and all
    "(B |\n// prettier-ignore\n(C[]{K}))",
    "(B &\n// format-ignore\n(G<T>{K}))",
    "(A extends B ? C :\n// prettier-ignore\n(D[]{K}))",
    "(keyof\n// prettier-ignore\n(C{K}))",
    "(readonly\n// prettier-ignore\n(C[]{K}))",
    "(B | (\n// prettier-ignore\nC{K}))",
    // a doubled pair
    "((C{K}))",
    "(((C{K})))",
    "((/* l */ C{K}))",
    "((string{K}))",
    "((this{K}))",
    "((void{K}))",
    "(({ a: C }{K}))",
    "(([C]{K}))",
    "(({ [P in K]: C }{K}))",
    "((A.C{K}))",
    "((G<T>{K}))",
    "((C[]{K}))",
    "(('s'{K}))",
    "((C{K}) /* y */)",
    "((C /* q */{K}))",
    "((keyof C{K}))",
    "((readonly [C]{K}))",
    "((readonly C[]{K}))",
    "(({ a: C /* i */ }{K}))",
    "(([C /* i */]{K}))",
    "(({ a: C /* i */ }{K}) /* d */)",
];

/// The arrow head ahead of the return type: a bare `:`, and a `:` with a `//` after it,
/// which hangs what follows.
const HEADS: &[&str] = &["(a): ", "(a): // p\n"];

/// A multi-line block and a line comment, each written last in the innermost shell.
const COMMENTS: &[&str] = &[" /* x\n */", " // x\n"];

/// TypeScript's wire carries no comments (acorn's), so the comment texts are read from the
/// parse and set beside the tree, in the shape Svelte's wire gives its root `comments` list.
fn ts_wire(source: &str) -> Option<Value> {
    let json = tsv_ts::parse_to_json(source).ok()?;
    let mut wire = tsv_debug::json::wire_value(json.as_bytes());
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena).expect("the wire's source parses");
    let comments = program
        .comments
        .iter()
        .map(|comment| serde_json::json!({ "value": comment.content(source) }))
        .collect();
    wire.as_object_mut()
        .expect("a Program object")
        .insert("comments".to_owned(), Value::Array(comments));
    Some(normalized(wire))
}

fn svelte_wire(source: &str) -> Option<Value> {
    let json = tsv_svelte::parse_to_json(source).ok()?;
    Some(normalized(tsv_debug::json::wire_value(json.as_bytes())))
}

/// The wire with positions dropped and every type grouping pair unwrapped. Svelte's wire also
/// attaches each comment to a node beside the root's `comments` list, and that attachment
/// follows the comment's printed place, so it is dropped too: the root list still holds every
/// comment in source order. Every string is compared with its whitespace runs collapsed, since
/// the print reindents a block comment's continuation lines (no shape here holds a string
/// whose whitespace is its meaning).
fn normalized(mut value: Value) -> Value {
    loop {
        let inner = match &value {
            Value::Object(map) if map.get("type") == Some(&Value::from("TSParenthesizedType")) => {
                map.get("typeAnnotation").cloned()
            }
            _ => None,
        };
        match inner {
            Some(inner) => value = inner,
            None => break,
        }
    }
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "start" | "end" | "leadingComments" | "trailingComments"
                    )
                })
                .map(|(key, child)| (key, normalized(child)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(normalized).collect()),
        Value::String(text) => Value::String(text.split_whitespace().collect::<Vec<_>>().join(" ")),
        other => other,
    }
}

/// Formats `source`, reparses the output and compares the two trees; returns a description of
/// the failure, if any.
fn grade(
    source: &str,
    format: fn(&str) -> tsv_lang::Result<String>,
    wire: fn(&str) -> Option<Value>,
) -> Option<String> {
    let before = wire(source).expect("the input parses");
    let out = match format(source) {
        Ok(out) => out,
        Err(error) => return Some(format!("{source:?}: format failed: {error}")),
    };
    match wire(&out) {
        None => Some(format!("{source:?}: output does not parse: {out:?}")),
        Some(after) if after != before => Some(format!(
            "{source:?}: output reads as another program: {out:?}"
        )),
        Some(_) => None,
    }
}

#[test]
fn arrow_return_type_shell_comment_output_reparses() {
    let mut failures = Vec::new();
    for head in HEADS {
        for shape in SHAPES {
            for comment in COMMENTS {
                let arrow = format!("{head}{} => 1", shape.replace("{K}", comment));
                let script = format!("x = {arrow};\n");
                let component = format!("<script lang=\"ts\">\n\tx = {arrow};\n</script>\n");
                // Svelte reads TypeScript in the template only beside a `lang="ts"` script
                let expression = format!("<script lang=\"ts\"></script>\n{{{arrow}}}\n");
                failures.extend(grade(&script, tsv_ts::format, ts_wire));
                failures.extend(grade(&component, tsv_svelte::format, svelte_wire));
                failures.extend(grade(&expression, tsv_svelte::format, svelte_wire));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failing cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A `readonly` operand's own pair holds a comment written inside it, so the operand keeps
/// that pair and the return type needs no outer one: `(readonly [C]) =>` is a parameter list
/// to tsc (the modifier is skipped and the tuple read as a binding pattern), a reading tsv's
/// reparse cannot see, and acorn-typescript rejects a `readonly` directly over a pair, so no
/// fixture can hold the shape. The printed form is pinned here instead.
#[test]
fn readonly_operand_pair_keeps_its_comment() {
    let out = tsv_ts::format("x = (a): (readonly ([C] // x\n)) => 1;\n").expect("formats");
    assert_eq!(out, "x = (\n\ta\n): readonly (\n\t[C] // x\n) => 1;\n");
}
