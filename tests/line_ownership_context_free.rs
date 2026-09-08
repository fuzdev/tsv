// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Line ownership, graded where no fixture reaches: the STANDALONE path's claim.
//!
//! `EmbedContext::printer_owns_line` says whether a break after a doc is still that doc's
//! own, and it licenses the member chain's trailing-member collapse (`fn() // c⏎.bar` →
//! `fn().bar; // c`, whose comment defers through `line_suffix` to a line end). A Svelte
//! TEMPLATE island does not own that line — past the closing `}` the document is markup, so
//! a deferred `//` comes out as rendered page text — and the field's default is `false` for
//! exactly that reason: the safe answer costs a collapse, the unsafe one corrupts a page.
//!
//! Two printers claim ownership, and they are graded in different places:
//!
//! - the Svelte **`<script>`** body (`tsv_svelte`'s `script_style.rs`) — pinned by the whole
//!   `<script lang="ts">` fixture corpus, which is where the collapse's own fixtures live
//!   ([`trailing_member_after_call_comment`](../tests/fixtures/typescript/expressions/calls/chained/trailing_member_after_call_comment_prettier_divergence/)
//!   and its siblings);
//! - the **standalone document** (`tsv_ts`'s `format_program_in`) — pinned by NOTHING. Every
//!   fixture that exercises this shape is a `.svelte` file, so deleting the standalone
//!   claim leaves all 4,555 of them green (measured), and a formatted corpus cannot see it
//!   either: the collapsed form has no comment in the gap at all, so re-formatting real
//!   `.ts` moves zero bytes whichever way the flag is set. The shape only exists in
//!   authoring.
//!
//! So this file grades the standalone half directly, through the invariant the crate already
//! promises: **TypeScript formats context-free** — identical output standalone and inside a
//! `<script lang="ts">` (`crates/tsv_ts/CLAUDE.md` §Distinctives). Dropping the standalone
//! claim breaks that promise rather than merely changing a layout, and this is the cheapest
//! instrument that says so. The opposite direction — a template island that must NOT
//! collapse — is a formatting claim with an oracle, so it stays a fixture
//! ([`svelte/expressions/chain_trailing_member_gap_line_comment`](../tests/fixtures/svelte/expressions/chain_trailing_member_gap_line_comment_prettier_divergence/)).

/// The body as `tsv_ts` formats a standalone document.
fn standalone(body: &str) -> String {
    tsv_ts::format_str(body).expect("standalone TS should parse")
}

/// The same body as `tsv_svelte` formats it inside a `<script lang="ts">`, dedented by the
/// one tab the wrapper adds, so it is comparable to the standalone output.
fn embedded(body: &str) -> String {
    let component = tsv_svelte::format_str(&format!("<script lang=\"ts\">\n{body}</script>\n"))
        .expect("component should parse");
    let inner = component
        .strip_prefix("<script lang=\"ts\">\n")
        .and_then(|rest| rest.strip_suffix("</script>\n"))
        .expect("the wrapper should round-trip");
    inner
        .lines()
        .map(|line| line.strip_prefix('\t').unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The authorings whose layout depends on line ownership, plus the neighbours that must not
/// move with them: a same-line `//` in a chain's call→member gap is the shape the collapse
/// licenses, an own-line one already forces the chain open, and a called tail never took the
/// collapse at all.
const CASES: &[(&str, &str)] = &[
    (
        "same-line gap comment, trailing member — the collapse the claim licenses",
        "const a = fn() // c\n\t.bar;\n",
    ),
    (
        "the same shape behind a call's arguments",
        "const a = fn(x, y) // c\n\t.bar;\n",
    ),
    (
        "own-line gap comment — forces the chain open on both paths",
        "const a = fn()\n\t// c\n\t.bar;\n",
    ),
    (
        "called tail — never eligible for the collapse",
        "const a = fn() // c\n\t.bar();\n",
    ),
    (
        "a longer chain — expanded on both paths",
        "const a = fn() // c\n\t.bar.baz.qux;\n",
    ),
];

#[test]
fn typescript_formats_the_same_standalone_and_in_a_script() {
    for (what, body) in CASES {
        let standalone_out = standalone(body);
        let embedded_out = embedded(body);
        assert_eq!(
            standalone_out, embedded_out,
            "context-free formatting broke for {what}\n\
             standalone:\n{standalone_out}\nin <script>:\n{embedded_out}"
        );
    }
}

#[test]
fn a_line_owning_printer_takes_the_trailing_member_collapse() {
    // The parity test above is satisfied by BOTH printers expanding, so it cannot tell a
    // dropped ownership claim from a deliberate retirement of the collapse. This pins which
    // answer the pair agrees on: a printer that owns its lines defers the `//` past the
    // member, which is the whole content of `printer_owns_line: true`.
    let body = "const a = fn() // c\n\t.bar;\n";
    assert_eq!(standalone(body), "const a = fn().bar; // c\n");
    assert_eq!(embedded(body), "const a = fn().bar; // c\n");
}
