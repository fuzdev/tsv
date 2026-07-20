# Comment Handling: The Detached Model

> The full doctrine for tsv's comment model — the `Comment` type, ownership, the three lookup axes, the hazards, and the leading-comment emitter rules. The always-loaded summary lives in [CLAUDE.md §Comment Handling](../CLAUDE.md#comment-handling-detached-model); read **this** doc before changing comment handling in any printer. The lookup API itself is documented in [`crates/tsv_lang/CLAUDE.md` §Comment Utilities](../crates/tsv_lang/CLAUDE.md#comment-utilities).

Comments are stored **separately from AST nodes** in a flat `Vec<Comment>` at the root level (`Program.comments`, `CssStyleSheet.comments`, `Root.comments`). The printer finds comments via O(log n) binary search on span positions.

**Core type** (`tsv_lang/src/comment.rs`):

```rust
pub struct Comment {
    pub content_span: Span,        // content WITHOUT delimiters; text via content(source)
    pub is_block: bool,
    pub multiline: bool,           // content contains '\n' (precomputed; block-only in practice)
    pub span: Span,                // full comment span, delimiters included
    pub emit_character_field: bool, // Serializer hint: include `character` in JSON loc
    pub bump_pattern_columns: bool, // Serializer hint: +1 loc columns (Svelte block-pattern parse)
    pub owned_by_node: bool,        // Printed by the node it's bound to, not by the enclosing gap
}
```

## Owned comments — the one crack in the detached model

A comment that is *bound to the token after it* can't be located positionally at print time, because a paren the printer synthesizes around an enclosing expression lands between the two and re-binds it. **Every glued block comment is owned** (`owned_by_node`, set by the parser): the position is an authoring choice that binds the comment to the operand it leads, so the operand's own doc prints it and no synthesized paren can land between them. There is no content sniff — a plain `/* c */` and a bundler annotation `/* @__PURE__ */` bind their token identically. Two shapes are worth naming:

- the **glued block comment** (the general case) — printed by the innermost node its token begins (`printer/comments/owned.rs`, via `build_expression_doc`'s `prepend_owned_leading_comment`). Covers an ordinary leading comment and a **bundler annotation** alike (`/* @__PURE__ */` and friends mark the call *after* them as side-effect-free; a paren between the two would leave the annotation leading a paren, so the marked call is no longer droppable). No AST node is involved.
- the **JSDoc cast** — `/** @type {T} */` plus the `(` it glues to **are** the cast, so the comment is handed to the `JsdocCast` node, which prints it. `is_jsdoc_type_cast_comment` is the **only** remaining content sniff, and it governs the cast's **paren-retention** (building the `JsdocCast`), *not* ownership — ownership flows to a cast the same as to any other glued comment.

An owned comment is always a **block** comment (`owned ⇒ is_block`), and always **glued** to its token — a comment the author left on its own line leads the *line*, not the token. The one exception is the JSDoc cast, whose comment may sit a newline above its `(` and still be the cast; that difference is load-bearing and named at the shared scan (`source_scan::CommentGlue`).

## Ownership is about who PRINTS, never whether a comment EXISTS

That one sentence is the whole rule, and every bug in this class has been a violation of it. A comment can be asked about along exactly **three** axes, and the lookup API (`tsv_lang::comment`) makes the caller name which:

| axis | question | owned comments | who asks |
| --- | --- | --- | --- |
| **to emit** | "which comments must *I* print here?" | **skipped** | gap emitters (~200 sites) |
| **on page** | "does any comment OCCUPY THE PAGE here?" | **counted** | layout gates — break / expand / hug / paren / fast-path |
| **in source** | "what comment BYTES are physically here?" | **counted** | cursors — blank-line scans, offsets, `prev_end` |

`comments_to_emit_in_range` / `has_comments_to_emit_in_range` / `comments_to_emit_after` · `comments_on_page_in_range` / `has_comments_on_page_in_range` / `has_multiline_block_comments_on_page_in_range` · `comments_in_source_range` / `comments_in_source_after`. Every name states its axis, so a miswire reads as a category error at the call site rather than as plausible code. Two facts about the shape:

- **Three questions, but only two membership sets.** *On page* and *in source* both count an owned comment (it is in the output, and its bytes are in the file); only *to emit* skips it. The two names exist because the *question* differs — and naming the wrong one is the bug.
- `has_line_comments_in_range` carries **no** axis, provably: `owned ⇒ is_block`, so no line comment is ever owned and skip ≡ count. If that ever changes, it must grow an axis.

Two corollaries worth naming, because each was a whole family of bugs:

- A **zero-comment fast gate** (`let has_comments = …` guarding a whole builder) is an **on-page** question. It short-circuits the layout gates it guards, so an emit-keyed one makes every one of them blind.
- A **blank-line scan** is an **in-source** question. `has_blank_line_between*` is a raw newline count — it cannot tell a comment's own newlines from an author's blank line, so the scan must step over every comment in the gap (`blank_scan_start` / `blank_scan_end`), not just the ones this caller emits.

## The three hazards

⚠️ All three have bitten. Ownership is a sharp tool: it takes a comment out of the positional model, so every consumer of that model has to be re-examined.

1. **An owned comment nothing prints is a DROPPED comment.** Ownership assumes the owning node prints it, so any builder that **reassembles** a node instead of routing it through `build_expression_doc` must claim it on its own seam (`prepend_owned_leading_comment_at`). Two do: `build_arrow_sig_doc` (every call-argument state) and `build_arrow_chain_doc`'s inner arrows. Adding a third reassembly path means adding a third claim.
2. **An owned comment travels *inside* its node's doc, so the gap around it can't see it.** The assignment layout inspects the operator→value gap (`rhs_comments`), which is empty for an owned comment — yet the comment still hangs the value. The node has to be asked instead: `owned_leading_comment_hangs_value`. It is the single seam for that question (the declarator, the class property, the object value, and the `is_poorly_breakable_chain` invariant all route through it).
3. **A region the parser LIFTS OUT of its container is still inside the container's gap** — so two emitters print it, where hazard 1 has none. `<svelte:element this={…}>` keeps its `this` out of `Element::attributes` (it rides in the kind), yet the braces still sit in the name→`>` gap the attribute scan probes: the tag's own doc prints the comment, then the scan prints it again. `AttrGaps::claimed` is that seam — the region whose comments the scan must skip. What makes this one nasty is that **ownership masks it**: a glued *block* comment is owned, so the gap skips it and the double-print never appears; only a **line** comment (never owned — `owned ⇒ is_block`) exposes it. A lifted region needs a claim on *both* axes, and testing with block comments alone will tell you it is fine.

All three hazards are what the **print-once comment ledger** exists to catch — the structural guard on this model: every comment a document parses must be emitted exactly once, or the audit reports it as DROPPED or DOUBLE-PRINTED (`deno task comments:audit`, gated in `deno task check`; see [audits.md §Comment Ledger Audit](./audits.md#comment-ledger-audit-commentsaudit)). Nothing else in the detached model forces a parsed comment to reach the output. Hazard 3 was found by it, not by review — the block-comment repro looked clean.

Prettier, oxfmt and biome all get the paren binding wrong — see [conformance_prettier.md §Comment relocation](./conformance_prettier.md#comment-relocation) and [§JSDoc / paren semantics](./conformance_prettier.md).

## Content is a source slice, never owned

The content is **not stored owned** — comment text is a pure delimiter-stripped sub-slice of source, so `Comment` holds a `content_span` and recovers the text on demand via `Comment::content(source) -> &str` (`source` must be the host document the spans were recorded against); every field is `Copy`, no `String` per comment. `multiline` is precomputed so the multi-line-block expansion checks (`has_multiline_block_comments_on_page_in_range` and the printers) read an O(1), source-free flag instead of re-scanning content. The full comment span includes its delimiters (`//` / `/* */` / a `#!` hashbang, whose content includes the `#!`); the lexer is the single owner of those widths.

## Printer strategy

Range-based lookup via `comments_to_emit_in_range(prev_end, node_start)` (and its on-page / in-source siblings above). Source string for context (same-line detection, blank line preservation). Tradeoff: simple/efficient AST matching Prettier's model, but printer must manually track `prev_end` positions; edge cases (e.g., arrow function comments) require careful span math.

## Leading comments: one rule, one emitter

A comment run *before* an item (a value, member, list element, or comma-separated item) is emitted by `Printer::push_leading_comment_run` (`printer/comments/mod.rs`), which implements prettier's `printLeadingComment` and picks the separator after each comment from the source around **that comment only**, never from where the item starts: **space** when nothing but spaces follow its `*/` (so a run the author glued stays glued — `/* a */ /* b */⏎X`), a soft **`line`** when a newline follows but none precedes, and a blank-preserving **`hardline`** for an own-line block or any line comment. The glue test alone is `Printer::comment_hugs_next` — the single statement of the rule, called even by the few sites whose surrounding loop must differ. The three hand-rolled leading-run sites whose loop can't route through `push_leading_comment_run` — `build_eq_comment_break_rhs`, `append_keyword_value_line_comments`, `emit_leading_comments_inline_aware` (all always-broken line-comment contexts, so a two-outcome space-or-hardline separator) — share `Printer::push_leading_run_separator`, which pairs the **physical-next** anchor (`blank_scan_end`, so an owned comment glued to the value doesn't desync the decision) with the `comment_hugs_next` hug-or-blank-hardline choice. ⚠️ Do not hand-roll `is_block && is_same_line(c.span.end, X)` at a new site, and don't re-derive the anchor+separator inline — keying the hug on the *item* rather than on *what follows the comment*, or anchoring on the emit-next *past* an owned comment, splits an author-glued run or inserts a phantom blank, and was a whole bug family (unglue / block-run merge / owned-comment blank scan). A site that also owns a comma emits the gap via `push_inter_item_line_comment_gap`, which owns the break too — that is what lets the last comment hug the next item.

## Array family vs params family: whether the soft `line` collapses

Whether that soft `line` collapses is decided by one fact, and it predicts every list site. The **array family** — array literals, array patterns, and tuple types — wraps each element's run *plus* the element in a per-element `group` (`Printer::build_list_element_group`), so the `line` is measured against that element alone and collapses (`/* c1 */ /* c2 */ a`) even while the list itself is broken. The **params family** — function / type-parameter / type-argument / call-argument lists — gives an element **no group of its own** (the width path is a bare `join([",", line])`; the comment-forced-multiline path is a hardline-joined list), so the identical `line` has nothing to be measured against but the enclosing broken group, and breaks (`/* c1 */ /* c2 */⏎a`). This mirrors prettier exactly: `printArrayElements` pushes `group(print())` per element and `print()` carries the leading comments (`print/array.js`), while `printFunctionParameters` / `printTypeParameters` / `print/call-arguments.js` do not. Don't re-derive it per site — and don't "fix" a params-family break by adding a group, or an array-family collapse by removing one.

## Beyond the detached model

Higher-fidelity models (attached comments, trivia tokens) may be needed for IDE/linter use cases.
