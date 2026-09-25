// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A `<script>` or `<style>` nested in Svelte markup formats its body exactly as the
//! top-level section does, one indent level per enclosing container deeper.
//!
//! Each case states its formatted body once, line by line, each line tagged with how it is
//! placed: a **structural** line is one the embedded printer breaks and indents (so it moves
//! with the body's indent level), a **verbatim** line is a continuation of text the printer
//! copies — a template literal's quasi, a non-indentable comment's interior, a
//! `prettier-ignore` slice — and keeps its authored column at every depth. The same body is
//! then formatted at the top level and nested at depths 1, 2 and 4 under several container
//! kinds, and every placement must match the one statement: structural lines at the body's
//! level, verbatim lines untouched, and the nested output a fixed point.
//!
//! The chain case is the one where the embedding CONTEXT shows, not just the indentation: a
//! `<script>` body owns its lines (`tsv_lang::EmbedContext::line_owning`), so a `//` in the
//! call→member gap defers to the statement's end (`fn().bar; // c`) as it does at the top
//! level, where a template island's default context would expand the chain instead.

/// How one line of a formatted body is placed.
#[derive(Clone, Copy)]
enum Line {
    /// Broken and indented by the printer: `extra` levels past the body's own.
    Structural(usize, &'static str),
    /// Copied text: printed exactly as written, at every depth.
    Verbatim(&'static str),
}

use Line::{Structural as S, Verbatim as V};
use std::fmt::Write as _;

struct Case {
    name: &'static str,
    tag: &'static str,
    /// The authored body, placed between the tags as is.
    body: &'static str,
    expected: &'static [Line],
}

const CASES: &[Case] = &[
    Case {
        name: "member gap line comment",
        tag: "script",
        body: "fn() // c\n.bar;",
        expected: &[S(0, "fn().bar; // c")],
    },
    Case {
        name: "template literal",
        tag: "script",
        body: "const a = `x\n  y   \n\nz`;",
        expected: &[S(0, "const a = `x"), V("  y   "), V(""), V("z`;")],
    },
    Case {
        name: "prettier-ignore statement, the directive glued to the open tag",
        tag: "script",
        body: "// prettier-ignore\nconst b = [\n  1,   2,\n      3];\nfn( );",
        expected: &[
            S(0, "// prettier-ignore"),
            S(0, "const b = ["),
            V("  1,   2,"),
            V("      3];"),
            S(0, "fn();"),
        ],
    },
    Case {
        name: "trailing prettier-ignore stays inert",
        tag: "script",
        body: "a; // prettier-ignore\nb  =  1;",
        expected: &[S(0, "a; // prettier-ignore"), S(0, "b = 1;")],
    },
    Case {
        name: "blank-line run",
        tag: "script",
        body: "const a = 1;\n\n\n// c\n\nconst b = 2;",
        expected: &[
            S(0, "const a = 1;"),
            V(""),
            S(0, "// c"),
            V(""),
            S(0, "const b = 2;"),
        ],
    },
    Case {
        name: "comment-only body",
        tag: "script",
        body: "/* a\nb */",
        expected: &[S(0, "/* a"), V("b */")],
    },
    Case {
        name: "multi-line comment",
        tag: "style",
        body: "/* a\nb */ .c{color:red}",
        expected: &[
            S(0, "/* a"),
            V("b */"),
            S(0, ".c {"),
            S(1, "color: red;"),
            S(0, "}"),
        ],
    },
    Case {
        name: "prettier-ignore rule, the directive glued to the open tag",
        tag: "style",
        body: "/* prettier-ignore */\n.a   {\n  color:red;\n      }\n.b{color:red}",
        expected: &[
            S(0, "/* prettier-ignore */"),
            S(0, ".a   {"),
            V("  color:red;"),
            V("      }"),
            S(0, ".b {"),
            S(1, "color: red;"),
            S(0, "}"),
        ],
    },
    Case {
        name: "blank lines between declarations and rules",
        tag: "style",
        body: ".a{color:red;\n\nmargin:0}\n\n\n.b{color:red}",
        expected: &[
            S(0, ".a {"),
            S(1, "color: red;"),
            V(""),
            S(1, "margin: 0;"),
            S(0, "}"),
            V(""),
            S(0, ".b {"),
            S(1, "color: red;"),
            S(0, "}"),
        ],
    },
];

/// Container chains, outermost first, each `(open, close)` adding one level.
const HOSTS: &[&[(&str, &str)]] = &[
    &[("<div>", "</div>")],
    &[("{#if a}", "{/if}")],
    &[("<svelte:head>", "</svelte:head>")],
    &[("{#each items as item}", "{/each}")],
    &[("<div>", "</div>"), ("<section>", "</section>")],
    &[("{#if a}", "{/if}"), ("<div>", "</div>")],
    &[("<svelte:head>", "</svelte:head>"), ("<div>", "</div>")],
    &[("{#each items as item}", "{/each}"), ("{:else}", "")],
    &[
        ("<div>", "</div>"),
        ("<section>", "</section>"),
        ("<article>", "</article>"),
        ("<aside>", "</aside>"),
    ],
    &[
        ("{#each items as item}", "{/each}"),
        ("{#if a}", "{/if}"),
        ("<div>", "</div>"),
        ("<span>", "</span>"),
    ],
    // Whitespace-sensitive ancestors: the body indents one level per container there too.
    &[
        ("<pre>", "</pre>"),
        ("<svelte:boundary>", "</svelte:boundary>"),
    ],
    &[
        ("<div>", "</div>"),
        ("<pre>", "</pre>"),
        ("<svelte:element this=\"div\">", "</svelte:element>"),
    ],
    &[
        ("{#if a}", "{/if}"),
        ("{#key k}", "{/key}"),
        ("<Comp>", "</Comp>"),
        ("{#snippet fn()}", "{/snippet}"),
    ],
];

/// The body's expected lines at indent `level`.
fn render(expected: &[Line], level: usize) -> Vec<String> {
    expected
        .iter()
        .map(|line| match *line {
            S(extra, text) => format!("{}{text}", "\t".repeat(level + extra)),
            V(text) => text.to_owned(),
        })
        .collect()
}

fn format(source: &str, label: &str) -> String {
    let formatted = tsv_svelte::format_str(source);
    assert!(formatted.is_ok(), "{label}: format failed: {formatted:?}");
    formatted.expect("asserted Ok above")
}

/// The lines between the first `<tag>` line and its `</tag>` line, asserting both tags sit
/// at `depth`.
fn body_lines(output: &str, tag: &str, depth: usize, label: &str) -> Vec<String> {
    let lines: Vec<&str> = output.lines().collect();
    let indent = "\t".repeat(depth);
    let (open, close) = (format!("{indent}<{tag}>"), format!("{indent}</{tag}>"));
    let start = lines.iter().position(|l| *l == open);
    let end = lines.iter().position(|l| *l == close);
    assert!(
        start.is_some() && end.is_some(),
        "{label}: no `{open}` … `{close}` pair in:\n{output}"
    );
    let (start, end) = (start.expect("asserted above"), end.expect("asserted above"));
    lines[start + 1..end]
        .iter()
        .map(|l| (*l).to_owned())
        .collect()
}

#[test]
fn top_level_body_matches_the_statement() {
    for case in CASES {
        let label = format!("{} at top level", case.name);
        let source = format!("<{0}>{1}</{0}>\n", case.tag, case.body);
        let output = format(&source, &label);
        assert_eq!(
            body_lines(&output, case.tag, 0, &label),
            render(case.expected, 1),
            "{label}:\n{output}"
        );
    }
}

#[test]
fn nested_body_matches_the_top_level_one_level_per_container_deeper() {
    for case in CASES {
        for host in HOSTS {
            // `{:else}` is a section of the block before it, not a container of its own:
            // the body sits in the `{#each}`'s fallback, one level in.
            let depth = host.iter().filter(|(_, close)| !close.is_empty()).count();
            let mut source = String::new();
            for (open, _) in *host {
                source.push_str(open);
            }
            write!(source, "<{0}>{1}</{0}>", case.tag, case.body).expect("String write");
            for (_, close) in host.iter().rev() {
                source.push_str(close);
            }
            source.push('\n');
            let label = format!("{} nested in {source:?}", case.name);

            let output = format(&source, &label);
            assert_eq!(
                body_lines(&output, case.tag, depth, &label),
                render(case.expected, depth + 1),
                "{label}:\n{output}"
            );
            assert_eq!(
                format(&output, &label),
                output,
                "{label}: not a fixed point"
            );
        }
    }
}
