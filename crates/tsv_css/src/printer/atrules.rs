// CSS at-rule formatting
//
// Handles formatting of:
// - At-rules (@media, @keyframes, @supports, @import, @layer, @font-face, etc.)
// - At-rule blocks and their children (rules, declarations, nested at-rules)
//
// ## Architecture
//
// Doc-first (like `selectors.rs`/`values.rs`): each prelude is built as one doc tree
// and rendered through the renderer via the shared `write_arena_doc_with_suffix` (the
// same writer selectors.rs uses), so the wrap decision and emission share a single
// representation — no measurement pass to drift from emission. `@supports`/`@container`
// conditions and the `@media`/`@import` single-query `and`/`or` wrap are `fill`s; the
// `@media` comma list is a `group`. The one holdout is the `@import` comma-separated
// query *list* (`print_import_media_query_fill`), whose two-level greedy fill keeps the
// first query on the `@import` line and breaks it internally — tsv's renderer fill moves
// an over-wide first item to its own line instead (the load-bearing `at_line_start`
// divergence kept for Svelte), so that one prelude stays imperative. The block body is
// iterated by the shared `print_css_block_children` (see `mod.rs`).

use std::borrow::Cow;

use super::Printer;
use super::boundary_ws::closer_pos;
use super::selectors::first_anchor;
use super::value_normalization;
use super::value_normalization::ValueReader;
use crate::ast::internal;
use crate::whitespace::Edge;
use tsv_lang::Span;
use tsv_lang::doc::{DocBuf, DocContext, arena::DocId};
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};

/// Whether `atom` is a media-query `and`/`or` connector (ASCII case-insensitive per
/// CSS Syntax 3, so `AND`/`Or` count too). The connector is the line-wrap break
/// point in a `@media`/`@import` query; its **case is preserved** in output
/// (matching prettier — the author's `AND`/`and` is kept), so this is detection-only.
fn is_media_connector(atom: &str) -> bool {
    atom.eq_ignore_ascii_case("and") || atom.eq_ignore_ascii_case("or")
}

/// The `<media-query-list>` a normalized `@media`/`@import` prelude holds.
struct MediaQueryList<'a> {
    /// The prelude text to print — `content` with a *deletable* closing comma removed.
    text: &'a str,
    /// The list's entries, in order. An empty one is a real entry.
    entries: Vec<&'a str>,
    /// Whether the entries can't be spelled without a comma after the last one.
    closing_comma: bool,
}

/// Read a normalized `@media`/`@import` prelude as the `<media-query-list>` it is.
///
/// mediaqueries-4 §Syntax parses the production by parsing "a comma-separated list of
/// component values, then parsing each entry as a `<media-query>`" — so **every stretch
/// between two top-level commas is an entry**, including an empty one. An entry that
/// matches no `<media-query>` is not dropped from the list; §"Error Handling" replaces
/// it with `not all`, which is why a grammar mismatch "does not wipe out an entire media
/// query list, just the problematic media query". Discarding an empty entry here would
/// shorten the authored list — a rewrite, not a normalization — so the entries come back
/// as authored and the callers render an empty one as the nothing it is, between its two
/// commas.
///
/// A **closing** comma is different: the same algorithm discards each separator and stops
/// once the input is empty, so it contributes no entry at all (and `split_args_by_comma`
/// produces none) — `screen,` and `screen` are the same list, and tsv deletes the comma,
/// exactly as it deletes a trailing comma in a declaration value (`transition: a,` → `a`)
/// and in a JS list. It is **kept** in the one case where the deletion would change the
/// list: when the last entry is empty. `@import url('a.css'),;` is the one-entry list
/// `not all`, and without its comma it is the **empty** list, which mediaqueries-4
/// evaluates to true — the import would flip from never applying to always.
///
/// A **declaration value** answers this differently: there the comma is authored content
/// and is always kept, since the value is matched against a grammar rather than split (see
/// `declarations::list_has_closing_comma`). The `<media-query-list>` is the one construct
/// where mediaqueries-4 §Syntax delegates to the split itself, which is what makes the
/// deletion parse-preserving.
///
/// Whether a closing comma is there at all comes from `has_closing_comma`, derived from
/// the split rather than from a second scan of the text — which also puts the comma at the
/// final byte, and that is what makes slicing it off safe.
///
/// Shared by both preludes so one list formats one way in both positions.
fn media_query_list(content: &str) -> MediaQueryList<'_> {
    let parts = value_normalization::split_args_by_comma(content);
    let ends_with_comma = value_normalization::has_closing_comma(content, &parts);
    // CSS whitespace only, escape-aware (`escapes::trim_css`): a non-ASCII space at an
    // entry's edge is the entry's own content — `print<NBSP>` is one identifier to
    // css-syntax-3, and the run beside a comma is the author's — where a Unicode `str::trim`
    // deleted it from every entry but the list's only one.
    let entries: Vec<&str> = parts.into_iter().map(crate::escapes::trim_css).collect();
    let closing_comma = ends_with_comma && entries.last().is_some_and(|entry| entry.is_empty());
    let text = if ends_with_comma && !closing_comma {
        // Deleting it leaves the same entries, so it is padding. The trim is
        // escape-aware: an escaped space before the comma is content, not padding.
        crate::escapes::trim_end_preserving_escape(&content[..content.len() - 1])
    } else {
        content
    };
    MediaQueryList {
        text,
        entries,
        closing_comma,
    }
}

/// Where an at-rule's prelude region ends: its block's `{`, or its statement's `;` — the far
/// end of the gap after the prelude's last token.
fn prelude_region_end(atrule: &internal::CssAtrule<'_>) -> u32 {
    atrule
        .block
        .as_ref()
        .map_or_else(|| closer_pos(atrule.span), |block| block.span.start)
}

/// What one side of a condition query's gap holds for the printer
/// (`Printer::condition_gap_side`).
enum GapSide {
    /// A boundary run, spelled with the gap's comments in place — the claim's own edges
    /// already decided, so the caller adds no separator of its own on those sides.
    Claimed(String),
    /// No run: the gap's comments, single-spaced (empty when there are none), which the
    /// caller pads as its seam's comment spelling says.
    Comments(String),
}

/// A `@supports`/`@container` condition prelude. The two differ only in that
/// `@supports` is value-parsed by prettier (so its numbers/strings normalize)
/// while `@container` is kept raw, and that `@container` carries a name. Pairing
/// them in one enum keeps those two facts in lockstep (no `name`-without-raw or
/// raw-without-name combinations).
#[derive(Clone, Copy)]
enum ConditionKind<'a> {
    Supports,
    Container { name: Option<&'a str> },
}

impl<'a> ConditionKind<'a> {
    /// `@supports` values are normalized (numbers + string quotes); `@container`
    /// preludes are emitted verbatim, matching prettier.
    fn normalizes(self) -> bool {
        matches!(self, ConditionKind::Supports)
    }

    /// The optional container name prefix (`@container sidebar (...)`).
    fn name(self) -> Option<&'a str> {
        match self {
            ConditionKind::Container { name } => name,
            ConditionKind::Supports => None,
        }
    }
}

/// The render context of an at-rule prelude's `and`/`or` fill (`@media`, `@supports`,
/// `@container`): the `;`/`{` reserve, and the head marked **glued** — the at-rule's name and
/// its space are written ahead of the doc, so nothing in the fill separates the first
/// segment from them, and the fill's fresh-line drop may not land in front of it. Without
/// the mark an over-wide first segment (a long feature value, a multi-line comment whose
/// first line overruns) dropped to its own line and left the name's space stranded as
/// trailing whitespace (`@media ⏎\t\t(min-width: …`). The head renders in place and the
/// wrap lands at the `and` after it (`prelude_first_segment_long_prettier_divergence`).
fn prelude_fill_context(suffix_width: usize) -> DocContext {
    DocContext::reserving(suffix_width).with_glued_lead(true)
}

impl<'a> Printer<'a> {
    /// Format a CSS at-rule (@media, @keyframes, @supports, etc.)
    pub(super) fn print_css_atrule(&mut self, atrule: &internal::CssAtrule<'_>) {
        // A block child's own `allow_comment_or_whitespace` juncture: the run before the `@`
        // was skipped by the parser and this head is rebuilt from parts, so nothing else can
        // carry it. A rule child gets the same claim from its selector's first compound
        // (`preserved_boundary_ws`) — this is the at-rule spelling of it, and the reason
        // `<NBSP>div {}` and `<NBSP>@media {}` no longer disagree. Unfloored on purpose: a
        // backward scan stops at the first non-whitespace byte, so the enclosing `{` / `}` /
        // `;` / `>` bounds it without the caller passing one.
        self.write_head_boundary_ws(atrule.span.start);
        self.write("@");
        // At-rule names are ASCII case-insensitive; lowercase for output (`@MEDIA`
        // → `@media`), matching prettier. Emit from the name's *source* span, not the
        // decoded `name`: an escaped name (`@m\A x`) decodes to a raw control char,
        // and emitting that verbatim would inject it into the output (content loss on
        // reparse). `lowercase_at_rule_name` leaves an escaped (`\`) name verbatim,
        // so the escape is preserved — matching how property names / preludes emit.
        let name_src = atrule.name_span.extract(self.source);
        self.write(&value_normalization::lowercase_at_rule_name(name_src));

        // Print prelude based on type
        match &atrule.prelude {
            internal::PreludeValue::Values { values, span } if !values.is_empty() => {
                self.write(" ");
                // Special handling for @import with media query (last value may need
                // wrapping). At-rule names are case-insensitive (`@IMPORT` too).
                let is_import = atrule.name.eq_ignore_ascii_case("import");
                // Track the source position after the previous value so comments in the
                // gaps (before the first value, between values) can be reconstructed.
                // Svelte strips these from the prelude string; prettier preserves them
                // with single-space padding, so we interleave them here.
                let mut prev_end = span.start;
                for (i, value) in values.iter().enumerate() {
                    // A value that opens with a comma is a `<media-query-list>` whose
                    // first query is empty; the comma glues to the component before it.
                    let glue_next =
                        self.source.as_bytes().get(value.span().start as usize) == Some(&b',');
                    self.write_import_gap_comments(prev_end, value.span().start, i > 0, glue_next);
                    // The verbatim tail of an `@import` prelude — its last value when the
                    // parser read one (`parse_import_prelude`): the media condition in the
                    // common case, but any value the author put after the structured head,
                    // or the whole prelude when it leads with no url/string. A bare `layer`
                    // keyword is the other `Identifier` and lands here only when it is last,
                    // where the normalization below is inert on it.
                    if is_import
                        && i == values.len() - 1
                        && let internal::CssValue::Identifier {
                            span: value_span, ..
                        } = value
                    {
                        // Normalize from source (comment-aware) so embedded comments and
                        // their spacing survive (`screen /* c */ and (...)`).
                        let normalized = value_normalization::normalize_css_whitespace(
                            value_span.extract(self.source),
                        );
                        // The whole media condition routes here, not only a wrappable one:
                        // this is the single place the prelude's value normalization runs,
                        // so a condition that skips it comes out raw (`(a: .50PX)` stayed
                        // `.50PX` where prettier gives `0.5px`, and the identical condition
                        // joined by an `and` normalized). `print_import_media_query` picks
                        // the layout for every shape itself — a one-entry list and a
                        // comment-bearing one take the `and`/`or` wrap, only a
                        // comment-free comma list takes the fill (which splits on
                        // whitespace and would shatter a `/* … */`).
                        self.print_import_media_query(&normalized);
                        prev_end = value.span().end;
                        continue;
                    }
                    // Doc-based formatting normalizes quotes and spacing. The last
                    // value is followed by the rule's `;`, so reserve it — a value
                    // group that can break (`supports(…)`, `layer(…)`) must break one
                    // column sooner, the same boundary the media-query fill above
                    // already measures for itself. Without it a 101-char
                    // `@import … supports(…);` sat one char over print width.
                    let suffix_width = usize::from(i == values.len() - 1 && atrule.block.is_none());
                    let doc = self.build_css_value_doc(value);
                    self.write_arena_doc_with_suffix(doc, suffix_width);
                    prev_end = value.span().end;
                }
                // Trailing comments between the last value and the `;` (e.g.
                // `@import 'a.css' /* c */;`) — or the gap claimed whole, flush against the
                // `;`, where it holds a boundary run (`@import 'a.css' <NBSP>;`).
                self.write_prelude_gap(prev_end, prelude_region_end(atrule), Edge::Flush);
            }
            internal::PreludeValue::Raw { content, span } if !content.is_empty() => {
                // `content` is already verbatim (internal whitespace + comments preserved,
                // outer-trimmed, `url()` inner-trimmed) from the parser's non-normalized
                // raw path, so it matches prettier as-is — no comment-spacing rewrite.
                // Embedded newlines survive under Svelte `<style>` because the CSS renders
                // at its final indent (no post-hoc line re-indent to compound them).
                //
                // The prelude's comments ride out inside `content`, never reaching a
                // comment emitter — see `tsv_lang::comment_ledger`. (They *are* registered
                // in the stylesheet's flat list; the parser records them while reading the
                // prelude and also bakes them into `content`.)
                #[cfg(feature = "comment_check")]
                tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);
                #[cfg(not(feature = "comment_check"))]
                let _ = span;
                self.write(" ");
                self.write(content);
            }
            internal::PreludeValue::Supports { condition, span } => {
                // @supports conditions are declarations, so prettier normalizes
                // their values (e.g. numbers); @container queries are left raw.
                self.write_condition_prelude(ConditionKind::Supports, condition, *span, atrule);
            }
            internal::PreludeValue::Container {
                name,
                condition,
                span,
            } => {
                self.write_condition_prelude(
                    ConditionKind::Container {
                        name: name.as_deref(),
                    },
                    condition,
                    *span,
                    atrule,
                );
            }
            internal::PreludeValue::Media { content, .. } if !content.is_empty() => {
                self.write(" ");
                self.print_media_prelude(content, atrule.block.is_some());
            }
            internal::PreludeValue::CustomSelector { name, list, span } => {
                self.write_custom_selector_prelude(*name, list, *span, atrule);
            }
            internal::PreludeValue::Selectors { root, limit, .. } => {
                self.write_scope_prelude(root.as_ref(), limit.as_ref(), atrule);
            }
            _ => {}
        }

        if let Some(block) = &atrule.block {
            self.write_block_open();
            // Format the block's children via the shared block-body routine (also
            // used by rule bodies). At-rule blocks have no pre-`{` comments — the
            // prelude owns that region — so `start_index` is 0. A nested rule flows
            // through the canonical `print_css_rule`, so it formats identically to a
            // top-level rule (no separate at-rule-block rule path to drift).
            self.indent_level += 1;
            // Inside an `@keyframes` block, `from`/`to` selectors are case-insensitive
            // keywords (lowercased by the selector printer); save/restore so a stray
            // nested context can't leak the flag.
            let was_in_keyframes = self.in_keyframes;
            self.in_keyframes = crate::parser::is_keyframes_atrule(atrule.name);
            // The same save/restore for `@utility`, whose body's values glue a trailing
            // `*` (see `Printer::in_utility_at_rule`).
            let was_in_utility = self.value_scope.in_utility_at_rule;
            self.value_scope.in_utility_at_rule =
                was_in_utility || atrule.name.eq_ignore_ascii_case("utility");
            self.print_css_block_children(block.children, 0);
            self.value_scope.in_utility_at_rule = was_in_utility;
            self.in_keyframes = was_in_keyframes;
            self.indent_level -= 1;

            // Every child ends with `\n` (declarations/comments inherently,
            // rules/at-rules via the routine's trailing newline), and an empty block
            // leaves the `{\n` that opened the line — so the buffer always ends with a
            // newline here, and the closing `}` is written at the outer indent.
            // Prettier renders an empty at-rule block as `{\n}` (no blank line inside).
            self.write_indent();
            self.write_block_tail_boundary_ws(block.children, block.span);
            self.write("}");
        } else {
            self.write(";");
        }
    }

    /// Format an `@media` prelude (doc-first).
    ///
    /// Unlike `@supports`/`@container`, `@media` keeps the raw prelude string
    /// (comments preserved inline). The whole prelude is one doc tree: a
    /// comma-separated media-query *list* (Media Queries 4 §"media query list")
    /// becomes a `group` that breaks at every top-level comma (one query per line,
    /// matching prettier); a single query becomes an `and`/`or` fill that wraps
    /// greedily at its `and`/`or` boundaries — one break for a query that overflows
    /// once, several for a very long one (a deliberate divergence — prettier never
    /// wraps a single query). The trailing ` {` is reserved so the boundary breaks at
    /// print width.
    /// ```css
    /// @media screen and (min-width: 768px) and (max-width: 1024px) and
    ///     (orientation: landscape) {
    /// ```
    fn print_media_prelude(&mut self, content: &str, has_block: bool) {
        let suffix_width = if has_block { " {".len() } else { 0 };
        let doc = self.build_media_prelude_doc(content, suffix_width);
        self.write_arena_doc_with_suffix(doc, suffix_width);
    }

    /// Build the doc tree for a media-reader prelude — `@media` or `@custom-media` (see
    /// `print_media_prelude`).
    fn build_media_prelude_doc(&self, content: &str, suffix_width: usize) -> DocId {
        let d = self.d();
        // Normalize numbers + string quotes in the raw prelude (`.5px` → `0.5px`,
        // `"x"` → `'x'`). Comments preserved. `@media` and `@custom-media` are the two
        // at-rules prettier hands to `parseMediaQuery`, so their preludes take the
        // `adjustNumbers` rules — the known-unit gate, no hex fold, `WORD_PART` deciding
        // what absorbs a number.
        let content = value_normalization::normalize_value_text(content, ValueReader::MediaQuery);
        // Lowercase media-feature *names* (`(MIN-WIDTH: …)` → `(min-width: …)`),
        // matching prettier; media types, `and`/`or`/`not`/`only`, and feature values
        // are preserved (see `lowercase_media_feature_names`). Keep the already-owned
        // `content` when nothing changed (the common no-uppercase case borrows).
        let content = match value_normalization::lowercase_media_feature_names(&content) {
            Cow::Borrowed(_) => content,
            Cow::Owned(s) => s,
        };

        let MediaQueryList {
            text,
            entries: queries,
            closing_comma,
        } = media_query_list(&content);

        if queries.len() > 1 {
            // Comma-separated query list: a group that breaks at every comma (one
            // query per line, one indent level). Each query is emitted verbatim — the
            // list breaks all-or-nothing, queries don't wrap internally.
            let mut parts = DocBuf::new();
            for (i, query) in queries.iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
                // Connector case is preserved (matching prettier), so the query is
                // emitted verbatim — no atom rewriting in the comma-list branch.
                parts.push(d.text_pooled(query));
            }
            // A list-closing comma takes no separator after it — one would strand a
            // space before the ` {`.
            if closing_comma {
                parts.push(d.text(","));
            }
            return d.group(d.indent(d.concat(&parts)));
        }

        // Single query: wrap greedily at its `and`/`or` boundaries. `text` already has a
        // deletable closing comma removed, and keeps one the entries need (`@media ,`).
        self.build_and_or_wrap_doc(text, suffix_width)
    }

    /// Build an `and`/`or`-wrapping fill for a single media query (raw string).
    ///
    /// Splits the query into segments at its top-level `and`/`or` keywords (paren-,
    /// quote- and comment-aware via `split_by_space_preserving_parens`) and joins
    /// them with breakable separators that keep the keyword on the line before the
    /// break (`… and⏎\t…`). A query with no top-level `and`/`or` has no break point,
    /// so it's emitted verbatim. Shared by `@media` and `@import` single queries.
    ///
    /// A connector is a break point only where it **joins two segments** — one before it
    /// and at least one atom after it — which is the grammar's own rule rather than a
    /// heuristic about malformed input: every connector production binds its operand to
    /// the right, in both grammars this printer serves. Media Queries 4 §"Syntax" has
    /// `<media-and> = and <media-in-parens>` and `<media-or> = or <media-in-parens>`, and
    /// CSS Conditional 3 §"@supports" spells the same shape (`<supports-in-parens> [ and
    /// <supports-in-parens> ]*`). It is also why `not` is not a connector here at all: it
    /// is `not <media-in-parens>`, a **prefix**, so it opens a segment instead of joining
    /// two.
    ///
    /// A trailing `and`/`or` is therefore the query's own last word, and emitting a
    /// separator for it strands a space where the query ends: `@import 'a.css' screen and;`
    /// closed on `screen and ;`. `@media` hides that, because
    /// [`Printer::write_block_open`] absorbs a trailing space while the `;` writer
    /// deliberately does not (a prelude's last byte can *be* a space — see
    /// [`Self::write_condition_prelude`]), so the guard belongs here at the source rather
    /// than at either tail. The leading end is the same question read from the other side,
    /// and `!segment.is_empty()` already answers it.
    fn build_and_or_wrap_doc(&self, query: &str, suffix_width: usize) -> DocId {
        let d = self.d();
        let atoms = value_normalization::split_by_space_preserving_parens(query);

        // Re-group atoms into segments split at `and`/`or` keyword atoms. The
        // connector rides the separator before the following segment.
        let mut fill_parts = DocBuf::new();
        let mut segment = d.pool_writer();
        let mut has_connector = false;
        for (i, atom) in atoms.iter().copied().enumerate() {
            // A `and`/`or` connector (case-insensitive) is the wrap break point;
            // its case is preserved (emit the original `atom`).
            if is_media_connector(atom) && !segment.is_empty() && i + 1 < atoms.len() {
                has_connector = true;
                fill_parts.push(segment.finish_text());
                segment = d.pool_writer();
                fill_parts.push(d.concat(&[d.text(" "), d.text_pooled(atom), d.line()]));
            } else {
                if !segment.is_empty() {
                    segment.push(' ');
                }
                segment.push_str(atom);
            }
        }
        if !segment.is_empty() {
            fill_parts.push(segment.finish_text());
        }

        // No `and`/`or` boundary → no break point, emit verbatim.
        if !has_connector {
            return d.text_pooled(query);
        }

        let fill = d.fill(&fill_parts);
        let fill = d.with_context(fill, prelude_fill_context(suffix_width));
        d.indent(fill)
    }

    /// Write a `@supports` / `@container` prelude — the name→prelude separator included,
    /// because whether there *is* one is part of the same question.
    ///
    /// A prelude with **no condition part** (`@supports;`, `@container b /* c */ {`) is
    /// what makes it one question, in two ways. The query carries nothing, so the prelude
    /// REGION is the only carrier its comments have — [`Self::build_condition_query_doc`]
    /// returns early on empty `parts`, which is what dropped them. And with nothing at all
    /// to emit there must be no separator either, or a blockless `@supports;` closes on
    /// `@supports ;`. The block form is already flush — [`Printer::write_block_open`]
    /// absorbs a trailing space — but the `;` writer deliberately does **not** trim one,
    /// because a prelude's last byte can BE a space (`@layer a\ ;`, an escape whose
    /// payload is that space), so the decision has to be made here rather than at the tail.
    ///
    /// A comment can only follow the container name in this branch: one *before* it is
    /// followed by the name token, which anchors a condition part, so the prelude is not
    /// partless at all (`@container /* c */ b {`).
    fn write_condition_prelude(
        &mut self,
        kind: ConditionKind<'_>,
        condition: &internal::ConditionQuery<'_>,
        span: Span,
        atrule: &internal::CssAtrule<'_>,
    ) {
        if condition.parts.is_empty() {
            // Bounds the prelude region rather than the condition: the span's end is the
            // empty condition's own and stops short of a trailing comment, and its start
            // can too, so the walk runs from the at-keyword (which no comment can sit
            // inside) to the block's `{` or the rule's `;`.
            let region_end = atrule
                .block
                .as_ref()
                .map_or(atrule.span.end, |b| b.span.start);
            let comments = self.comment_blocks_in_range(atrule.name_span.end, region_end);
            // A prelude that is *only* an operator (`@supports and;`) is partless too —
            // the operand it wanted is exactly the part that never came — so this branch
            // owns the run beside the name and the comments. All three read the same way:
            // each piece the prelude has takes one leading space, and one with no piece at
            // all takes none.
            let pieces = [
                kind.name(),
                (!comments.is_empty()).then_some(comments.as_str()),
                condition.trailing_operators,
            ];
            for piece in pieces.into_iter().flatten() {
                self.write(" ");
                self.write(piece);
            }
            return;
        }

        self.write(" ");
        self.print_condition_query(kind, condition, atrule, span);
    }

    /// Format an `@supports`/`@container` condition prelude (doc-first).
    ///
    /// The whole prelude is one doc tree rendered through the renderer, so the
    /// wrap decision and emission share a single representation (no measure pass to
    /// drift from emission). The condition parts join into a `fill` whose `line`s
    /// break at the `and`/`or` boundaries — the keyword stays on line 1, the
    /// condition wraps to line 2 — with the trailing ` {` reserved so the boundary
    /// breaks at print width. tsv wraps where prettier never does (a deliberate,
    /// cataloged divergence — see conformance_prettier_css.md §CSS: At-Rules):
    /// ```css
    /// @supports (display: grid) and (transform: rotate(45deg)) and (filter: blur(5px)) and
    ///     (flex: 1aaa) {
    /// ```
    fn print_condition_query(
        &mut self,
        kind: ConditionKind<'_>,
        condition: &internal::ConditionQuery<'_>,
        atrule: &internal::CssAtrule<'_>,
        prelude_span: Span,
    ) {
        // The partless prelude is `write_condition_prelude`'s, this function's one caller
        // — it owns the name→prelude separator too, for the same reason. Everything below
        // may therefore assume a part exists.
        debug_assert!(
            !condition.parts.is_empty(),
            "a partless condition prelude is write_condition_prelude's to print"
        );
        // Where the first part's gap begins: the at-rule name's end, or the container name's.
        // The prelude's span opens past a boundary run at its head (the reader steps it before
        // the span begins, like the canonical trim), so the gap is measured from the name,
        // never from the span — ahead of a container name it is a gap of its own, claimed here.
        let name_end_pos = if let Some(n) = kind.name() {
            let kept = self.spell_gap_items(
                atrule.name_span.end,
                prelude_span.start,
                Edge::Flush,
                Edge::Presence,
            );
            if !kept.is_empty() {
                self.write(&kept);
            }
            self.write(n);
            // Separate the name from its condition.
            self.write(" ");
            // The name opens the span, so it ends `n.len()` past the span's start.
            prelude_span.start + n.len() as u32
        } else {
            atrule.name_span.end
        };
        // The tail gap's far end: the block's `{`, or the statement's `;`.
        let region_end = prelude_region_end(atrule);

        let suffix_width = if atrule.block.is_some() {
            " {".len()
        } else {
            0
        };
        let doc = self.build_condition_query_doc(
            kind,
            condition,
            name_end_pos,
            (prelude_span, region_end),
            suffix_width,
        );
        self.write_arena_doc_with_suffix(doc, suffix_width);
    }

    /// Build the doc for an `@import` prelude's `supports()` call.
    ///
    /// The argument is the same `<supports-condition>` `@supports` takes, so its
    /// segments print through the same emitter — the `@supports` value normalization
    /// (numbers, hex, string quotes) and the `selector()` routing, both of which
    /// prettier applies here too.
    ///
    /// The function's parens are peeled back off the part. The parser folded them *in*
    /// (they are the condition part's own, which is what let the bare `<declaration>`
    /// alternative reuse the parenthesized-part grammar), but as part text they'd be
    /// unbreakable, and an over-width `supports()` has to break somewhere. Peeling
    /// restores the `group("(" indent(softline …) softline ")")` shape every other
    /// function value takes, so this position's break point is the ordinary one.
    pub(super) fn build_supports_condition_doc(
        &self,
        name: &str,
        condition: &internal::ConditionQuery<'_>,
    ) -> DocId {
        let d = self.d();
        // A `supports()` argument is exactly one part; an empty query means the parse
        // found nothing to record.
        let Some(part) = condition.parts.first() else {
            return d.text_pooled(name);
        };
        let last = part.segments.len().saturating_sub(1);
        let mut inner = DocBuf::with_capacity(part.segments.len());
        for (i, segment) in part.segments.iter().enumerate() {
            // The first and last segments are `Text` by construction — the part opens
            // on the `(` and closes on the `)` — so the strip guards never fire.
            let peeled = match segment {
                internal::ConditionSegment::Text(text) => {
                    let mut text: &str = text;
                    if i == 0 {
                        text = text.strip_prefix('(').unwrap_or(text);
                    }
                    if i == last {
                        text = text.strip_suffix(')').unwrap_or(text);
                    }
                    internal::ConditionSegment::Text(text)
                }
                selectors => *selectors,
            };
            inner.push(self.build_condition_segment_doc(ConditionKind::Supports, &peeled));
        }
        let inner = d.concat(&inner);
        d.group(d.concat(&[
            d.text_pooled(name),
            d.text("("),
            d.indent(d.concat(&[d.softline(), inner])),
            d.softline(),
            d.text(")"),
        ]))
    }

    /// Build the doc for one condition part segment.
    ///
    /// `@supports` values are number-normalized (`.5px` → `0.5px`) and their hex colors
    /// lowercased (`#FFF` → `#fff`); `@container` is emitted verbatim, both matching
    /// prettier. A `selector()` argument prints through the selector printer — the one
    /// a rule's own selector uses, so the same selector formats the same way in both
    /// positions. `remove_lines` keeps it on the prelude's line: a selector's combinator
    /// and pseudo-arg break points belong to a rule's selector list, not to a condition,
    /// whose own wrapping is the `and`/`or` fill.
    fn build_condition_segment_doc(
        &self,
        kind: ConditionKind<'_>,
        segment: &internal::ConditionSegment<'_>,
    ) -> DocId {
        let d = self.d();
        match segment {
            internal::ConditionSegment::Text(text) => {
                if kind.normalizes() {
                    d.text_pooled(&value_normalization::normalize_value_text(
                        text,
                        ValueReader::Value,
                    ))
                } else {
                    d.text_pooled(text)
                }
            }
            // The selector printer's own comma seam, so a comment beside a list comma
            // (`selector(.a, /* c */ .b)`) partitions the gap exactly as it does in a
            // rule's selector list, and the gaps between the parens and the list claimed as
            // a pseudo-class's argument list claims them (a boundary run at either end —
            // `selector(a <NBSP>)` — included); `remove_lines` keeps the argument on the
            // prelude's line.
            internal::ConditionSegment::Selectors { list, paren } => {
                d.remove_lines(self.build_paren_selector_list_inner(list, *paren))
            }
        }
    }

    /// Build the doc tree for an `@supports`/`@container` condition prelude.
    ///
    /// A single condition has no break point — it's a plain concat (head, content, tail),
    /// emitted inline like prettier. Two or more conditions become `indent(fill([...]))`:
    /// the connector (`and`/`or`) and any comments split around it ride a breakable
    /// separator (the connector stays on the line before the break), and the trailing
    /// ` {`/`;` is reserved via the fill's `trailing_reserve` so the boundary breaks at
    /// print width.
    ///
    /// Every gap of the query — ahead of each part, split at the part's connector run into
    /// the side before the run and the side after it, and the tail after the last part — is
    /// read by [`Self::condition_gap_side`]: its comments, or, where it holds a boundary run,
    /// its items spelled in place (`printer/boundary_ws.rs` §Who claims where).
    ///
    /// ⚠️ **The two arms differ in their BREAK POINTS and in nothing else**, and every bug
    /// this builder has had came of one arm quietly answering a shared question its own way
    /// (the single-part arm printed no connector at all while its twin did). The head is
    /// `leading_first` and the tail is [`Self::condition_tail_doc`], both reached from both
    /// arms; a question that is not about where the line breaks belongs in one of those, not
    /// inline in an arm.
    fn build_condition_query_doc(
        &self,
        kind: ConditionKind<'_>,
        condition: &internal::ConditionQuery<'_>,
        name_end_pos: u32,
        (prelude_span, region_end): (Span, u32),
        suffix_width: usize,
    ) -> DocId {
        let d = self.d();
        let parts = condition.parts;

        let content_doc = |part: &internal::ConditionPart<'_>| {
            let mut segments = DocBuf::with_capacity(part.segments.len());
            for segment in part.segments {
                segments.push(self.build_condition_segment_doc(kind, segment));
            }
            d.concat(&segments)
        };

        // The first part's head: the gap between the name and the part — its connector run
        // included, since a connector with no operand on its LEFT still binds the part on
        // its right (`@supports and (a: b)`, `@container name and (a: b)`). Split at that
        // run like every later gap, so its comments and runs stay on their authored side.
        let leading_first = |part: &internal::ConditionPart<'_>| -> DocId {
            let mut head = DocBuf::new();
            match part.connector {
                None => self.push_condition_gap_ahead(&mut head, name_end_pos, part.span.start),
                Some(connector) => {
                    // Against the connector the claim keeps the author's separation, like every
                    // side below: a run glued to its FRONT (`<NBSP>and`) is one identifier to
                    // css-syntax-3, and a space here would split it into a run and a keyword.
                    self.push_condition_gap_ahead(&mut head, name_end_pos, connector.span.start);
                    head.push(d.text_pooled(connector.text));
                    // ⚠️ The space is the NAME SEPARATOR, not decoration: a connector is an
                    // identifier, and `read_identifier` takes every code point at or above
                    // U+00A0 as content, so a boundary run emitted flush after one re-parses as
                    // the single name `and<NBSP>` — a prelude that then falls to the raw path,
                    // its condition unreadable, and a fixed point either way.
                    // `printer/boundary_ws.rs` §How a claim is spelled states the rule for the
                    // family this juncture joined.
                    head.push(d.text(" "));
                    // The claim ends as the author did against the part: spaced or glued.
                    self.push_condition_gap_ahead(&mut head, connector.span.end, part.span.start);
                }
            }
            head.push(content_doc(part));
            d.concat(&head)
        };

        if parts.len() <= 1 {
            // Single condition: no break point, emit inline (leading + content + trailing).
            // A partless query never reaches here — `write_condition_prelude` answers it
            // (a prelude with nothing to print also has no separator to write, and this
            // builder cannot see the region its comments live in); the `debug_assert` at
            // that entry point states it. This arm is the release-mode floor, never the
            // handler.
            let Some(first) = parts.first() else {
                return d.text("");
            };
            let head = leading_first(first);
            return match self.condition_tail_doc(first, prelude_span, region_end, condition) {
                Some(tail) => d.concat(&[head, tail]),
                None => head,
            };
        }

        // Multiple conditions: a fill whose separators carry the connectors.
        let mut fill_parts = DocBuf::new();
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                fill_parts.push(leading_first(part));
                continue;
            }
            // Separator: ` <before-gap>? <connector> line`. The connector and whatever stood
            // before it stay on the previous line; the `line` breaks before the content, and
            // whatever stood after the connector leads the content on the next line.
            //
            // A claimed side keeps the author's separation on BOTH its edges — spaced or glued to
            // the previous part, and to whatever follows it: a run glued to a connector's front
            // (`<NBSP>and`) or to a part's (`<PS>selector(…)`) is one identifier with it to
            // css-syntax-3, so no separator may be written between them.
            let gap_start = parts[i - 1].span.end;
            let before_end = part.connector.map_or(part.span.start, |c| c.span.start);
            let mut sep = DocBuf::new();
            let mut chunk = DocBuf::new();
            let before =
                self.condition_gap_side(gap_start, before_end, Edge::Presence, Edge::Presence);
            if let Some(connector) = part.connector {
                match before {
                    GapSide::Claimed(kept) => sep.push(d.text_pooled(&kept)),
                    GapSide::Comments(comments) if comments.is_empty() => sep.push(d.text(" ")),
                    GapSide::Comments(comments) => {
                        sep.push(d.text(" "));
                        sep.push(d.text_pooled(&comments));
                        sep.push(d.text(" "));
                    }
                }
                // Emit the connector run's source text (`AND` stays `AND`), preserved like
                // prettier.
                sep.push(d.text_pooled(connector.text));
                sep.push(d.line());
            } else {
                match before {
                    // With no connector the claim is the whole gap between two parts, and the
                    // break goes where the author separated: before the part when they spaced
                    // the run from it, else after the previous part (the run then leads the
                    // part on the next line, glued to it), else nowhere.
                    GapSide::Claimed(kept) => {
                        if let Some(spaced) = kept.strip_suffix(' ') {
                            sep.push(d.text_pooled(spaced));
                            sep.push(d.line());
                        } else if let Some(glued) = kept.strip_prefix(' ') {
                            sep.push(d.line());
                            chunk.push(d.text_pooled(glued));
                        } else {
                            // Glued at both ends: the previous part, the run and this part are
                            // one token run to css-syntax-3 (`(a: b)<NBSP>selector(` holds the
                            // function token `<NBSP>selector(`), so they must not become two fill
                            // items at all — the fill breaks at EVERY item boundary it has to,
                            // whatever the separator holds. They extend the previous item.
                            if let Some(previous) = fill_parts.pop() {
                                fill_parts.push(d.concat(&[
                                    previous,
                                    d.text_pooled(&kept),
                                    content_doc(part),
                                ]));
                                continue;
                            }
                            sep.push(d.text_pooled(&kept));
                        }
                    }
                    GapSide::Comments(comments) if comments.is_empty() => sep.push(d.line()),
                    GapSide::Comments(comments) => {
                        sep.push(d.text(" "));
                        sep.push(d.text_pooled(&comments));
                        sep.push(d.line());
                    }
                }
            }
            fill_parts.push(d.concat(&sep));

            if let Some(connector) = part.connector {
                self.push_condition_gap_ahead(&mut chunk, connector.span.end, part.span.start);
            }
            chunk.push(content_doc(part));
            fill_parts.push(d.concat(&chunk));
        }

        // The query's tail rides the last part's line, exactly as it rides the only part's
        // in the single-part arm above.
        if let Some(tail) = parts
            .last()
            .and_then(|last| self.condition_tail_doc(last, prelude_span, region_end, condition))
            && let Some(last_chunk) = fill_parts.pop()
        {
            fill_parts.push(d.concat(&[last_chunk, tail]));
        }

        let fill = d.fill(&fill_parts);
        let fill = d.with_context(fill, prelude_fill_context(suffix_width));
        d.indent(fill)
    }

    /// One side of a condition query's gap — `[from, to)` — read the one way every gap of
    /// the query is: where it holds a boundary MEMBER, its items spelled in place
    /// ([`Self::spell_gap_items`], which then prints the gap's comments too), and otherwise
    /// its comments, single-spaced, for the caller to pad.
    ///
    /// The condition prelude's claim (`printer/boundary_ws.rs` §Who claims where). The parser
    /// steps a run at every one of these gaps (`skip_gap_registering_comments`), so each is a
    /// place the run has to come back out; `lead` and `trail` are the gap's edges, which only
    /// the caller knows.
    fn condition_gap_side(&self, from: u32, to: u32, lead: Edge, trail: Edge) -> GapSide {
        if self.gap_holds_member(from, to) {
            GapSide::Claimed(self.spell_gap_items(from, to, lead, trail))
        } else {
            GapSide::Comments(self.comment_blocks_in_range(from, to))
        }
    }

    /// Push the condition gap `[from, to)` onto `docs` AHEAD of what follows it — a part, or
    /// the connector run that binds one ([`Self::condition_gap_side`]): claimed flush on its
    /// left and with the author's separation against what follows, or its comments with the
    /// one space that separates them from it.
    fn push_condition_gap_ahead(&self, docs: &mut DocBuf, from: u32, to: u32) {
        let d = self.d();
        match self.condition_gap_side(from, to, Edge::Flush, Edge::Presence) {
            GapSide::Claimed(kept) => docs.push(d.text_pooled(&kept)),
            GapSide::Comments(comments) if comments.is_empty() => {}
            GapSide::Comments(comments) => {
                docs.push(d.text_pooled(&comments));
                docs.push(d.text(" "));
            }
        }
    }

    /// The query's own tail, to append to whatever printed its LAST part: the run of
    /// operators with no operand ([`Self::trailing_operators_doc`]), then the gap standing
    /// between the query and the end of the prelude — its comments, or its items where it
    /// holds a boundary run.
    ///
    /// One emitter because [`Self::build_condition_query_doc`]'s two arms ask the same two
    /// questions of the same stretch, and an arm that answers one of them its own way is
    /// exactly the drift this prelude has already paid for once — the single-part arm emitted
    /// no connector at all while its twin did. The two arms differ in where the tail LANDS (a
    /// concat, or the fill's last chunk), which is all they should differ in.
    ///
    /// The gap starts where the query's text ends: the last part, or — when the query ends on
    /// an unbound operator run — the run, whose own text carries everything the reader
    /// collected up to its last item and whose end the query's span reaches. Its far end is
    /// the block's `{` or the statement's `;` (`region_end`).
    ///
    /// `None` when the query has neither, so a caller need not concat an empty doc.
    fn condition_tail_doc(
        &self,
        last: &internal::ConditionPart<'_>,
        prelude_span: Span,
        region_end: u32,
        condition: &internal::ConditionQuery<'_>,
    ) -> Option<DocId> {
        let d = self.d();
        let mut tail = DocBuf::new();
        let gap_start = if condition.trailing_operators.is_some() {
            prelude_span.end.max(last.span.end)
        } else {
            last.span.end
        };
        if let Some(operators) = self.trailing_operators_doc(condition) {
            tail.push(operators);
        }
        match self.condition_gap_side(gap_start, region_end, Edge::Presence, Edge::Flush) {
            GapSide::Claimed(kept) => tail.push(d.text_pooled(&kept)),
            GapSide::Comments(comments) if comments.is_empty() => {}
            GapSide::Comments(comments) => {
                tail.push(d.text(" "));
                tail.push(d.text_pooled(&comments));
            }
        }
        (!tail.is_empty()).then(|| d.concat(&tail))
    }

    /// The tail carrying the query's run of operators with no operand, if it has one —
    /// ` ` then the run verbatim, to append to whatever printed the last part.
    ///
    /// **No break point**: a `line` here would be a wrap opportunity with nothing on its
    /// far side, the stranded separator [`Self::build_and_or_wrap_doc`] refuses for the
    /// same reason on the text path — a connector is a break point only where it joins
    /// two segments.
    ///
    /// It can never collide with the tail gap's claim beside it. A comment inside the run
    /// is carried *by* the run (the parser un-registers it there), and the gap the claim
    /// reads starts where the run ends.
    fn trailing_operators_doc(&self, condition: &internal::ConditionQuery<'_>) -> Option<DocId> {
        let trailing_operators = condition.trailing_operators?;
        let d = self.d();
        Some(d.concat(&[d.text(" "), d.text_pooled(trailing_operators)]))
    }

    /// Reconstruct comments sitting in an `@import` prelude gap (before a value).
    ///
    /// Svelte strips comments from the `@import` prelude string but the printer
    /// preserves them with single-space padding, matching prettier
    /// (`@import /* c */ url('a.css')`, `url('a.css') /* c */ screen`). When the gap
    /// holds no comment, only the inter-value separator space is emitted (when
    /// `needs_separator`, i.e. this isn't the first value — the leading `@import `
    /// space is already written).
    ///
    /// `glue_next` drops the space on the value's side of the gap: the value that
    /// follows opens with a comma (a `<media-query-list>` whose first query is empty),
    /// and a comma that *follows* something takes no space before it — the same rule
    /// `normalize_css_whitespace` applies to every comma inside a value, applied here at
    /// the seam between two prelude values. So `supports(a), screen` glues, and a comment
    /// in that gap keeps its leading space but not its trailing one
    /// (`supports(a) /* c */, screen`). A comma with no prelude value before it is a
    /// different position and keeps the at-keyword's separator (`@media , screen`);
    /// gluing there would read as an at-rule named `@media,`.
    ///
    /// A gap holding a boundary run the parser stepped (`@import <NBSP> 'a.css'`,
    /// `'a.css' <NBSP> screen`) is claimed whole instead, its comments in place
    /// ([`Self::spell_gap_items`]): the author's separation kept against the value before it
    /// and the one after it (flush against a comma-led value), where padding the comments
    /// would space one off a member it was glued to.
    fn write_import_gap_comments(
        &mut self,
        start: u32,
        end: u32,
        needs_separator: bool,
        glue_next: bool,
    ) {
        if self.gap_holds_member(start, end) {
            // The first value's gap follows the ` ` the prelude writer put after `@import`.
            let lead = if needs_separator {
                Edge::Presence
            } else {
                Edge::Flush
            };
            let trail = if glue_next {
                Edge::Flush
            } else {
                Edge::Presence
            };
            let kept = self.spell_gap_items(start, end, lead, trail);
            self.write(&kept);
            return;
        }
        let comments: Vec<_> = self.comments_to_emit_between(start, end).collect();
        if comments.is_empty() {
            if needs_separator && !glue_next {
                self.write(" ");
            }
            return;
        }
        if needs_separator {
            self.write(" ");
        }
        for (i, comment) in comments.iter().enumerate() {
            if i > 0 {
                self.write(" ");
            }
            self.print_css_comment(comment);
        }
        if !glue_next {
            self.write(" ");
        }
    }

    /// Emit an `@scope` prelude: ` [(root)]? [to (limit)]?`, with any comment in its
    /// out-of-paren gaps re-emitted at its authored position, single-spaced.
    ///
    /// Both clauses are independently optional (css-cascade-6), so a bare `@scope { … }`
    /// writes no prelude and `@scope to (limit)` writes only the limit. The lists are nested
    /// context, so they don't wrap (same as `:is()`, `:where()`).
    ///
    /// Each clause's `paren` span recovers a comment leading/trailing the list *inside* the
    /// parens (the same wrapping the `:is()` args use). The out-of-paren prelude gaps —
    /// leading (`@scope /* c */ (.a)`), between the root `)` and `to`, between `to` and the
    /// limit `(`, and after the last `)` before the block `{` — re-emit their comments here
    /// too, normalized to a single space on each side (prettier freezes the source spacing;
    /// a cataloged divergence — see conformance_prettier_css.md §CSS: Comments), and each
    /// claims a boundary run the parser stepped there ([`Self::write_prelude_gap`]).
    fn write_scope_prelude(
        &mut self,
        root: Option<&internal::ScopeClause<'_>>,
        limit: Option<&internal::ScopeLimit<'_>>,
        atrule: &internal::CssAtrule<'_>,
    ) {
        // Right bound of the pre-`{` gap: the block's `{`, or the `;` of a block-less
        // `@scope` (not valid CSS, but accepted like any prelude).
        let block_start = prelude_region_end(atrule);
        // Each gap ahead of a `(` or `to` keeps the author's separation from it when a claim
        // prints it (`write_prelude_gap`), so the literal after it writes no space of its own;
        // the gap before the terminator is flush against it, the block opener spacing itself.
        let before_token = Edge::Presence;
        let before_end = Edge::Flush;

        // Leading gap: the first structural token after `@scope` is the root `(`, else `to`,
        // else the block `{`. Its left bound is the at-rule name's end.
        let first_start = root
            .map(|r| r.paren.start)
            .or_else(|| limit.map(|l| l.to_span.start))
            .unwrap_or(block_start);
        let lead_edge = if first_start == block_start {
            before_end
        } else {
            before_token
        };
        let mut claimed = self.write_prelude_gap(atrule.name_span.end, first_start, lead_edge);

        if let Some(root) = root {
            self.write_scope_clause(root, !claimed);
        }
        if let Some(limit) = limit {
            // Between-clause gap: root `)` → `to` (only when a root precedes it).
            if let Some(root) = root {
                claimed = self.write_prelude_gap(root.paren.end, limit.to_span.start, before_token);
            }
            self.write(if claimed { "to" } else { " to" });
            // After-`to` gap: `to` → limit `(`. A run glued to `to` stays glued (the prelude
            // reader splits the keyword off it — `scope_to_keyword_len`).
            claimed =
                self.write_prelude_gap(limit.to_span.end, limit.clause.paren.start, before_token);
            self.write_scope_clause(&limit.clause, !claimed);
        }
        // Pre-`{` gap: after the last clause's `)` (only when a clause exists — a
        // bare `@scope /* c */ {` comment is the leading gap above).
        if let Some(last_end) = limit
            .map(|l| l.clause.paren.end)
            .or_else(|| root.map(|r| r.paren.end))
        {
            self.write_prelude_gap(last_end, block_start, before_end);
        }
    }

    /// Emit a `@custom-selector` prelude: ` :--name <selector-list>`, with any comment in
    /// its gaps re-emitted at its authored position, single-spaced.
    ///
    /// Prettier's shape (`selector-root` under `insideAtRuleNode(path, "custom-selector")`):
    /// `indent([" ", group([customSelector, line, join([",", line], selectors)])])` — one
    /// group, so a list that fits stays inline and one that does not breaks **every**
    /// `line`: the name keeps the at-rule's line and each selector takes its own, indented
    /// once, and a selector that still overflows breaks at its combinators one level deeper
    /// (its own `group(indent(…))`, `build_complex_selector_doc`). The list is the selector
    /// printer's own comma seam (`build_comma_list_doc`, breakable), so a comment beside a
    /// comma partitions there as it does in a rule head. The `;` (or ` {`) after the group
    /// is the suffix the fit measures against.
    ///
    /// Gap comments: the leading gap (`@custom-selector /* c */ :--a`) and the trailing
    /// one (`h2 /* c */;`) sit outside the group and write through
    /// `write_prelude_gap`; the name→list gap (`:--a /* c */ h1`) is inside it,
    /// ahead of the `line`, so a broken list keeps the comment on the name's line.
    /// Prettier drops every one of them — a cataloged divergence
    /// (conformance_prettier_css.md §CSS: Comments).
    fn write_custom_selector_prelude(
        &mut self,
        name: Span,
        list: &internal::SelectorList<'_>,
        span: Span,
        atrule: &internal::CssAtrule<'_>,
    ) {
        // Leading gap: from the at-rule name's end — the `@custom-selector` token can hold
        // no comment, so this never reaches into the name — to the `:--name`.
        let claimed = self.write_prelude_gap(atrule.name_span.end, name.start, Edge::Presence);

        let d = self.d();
        let mut parts = DocBuf::new();
        parts.push(d.text_pooled(name.extract(self.source)));
        // The name→list gap is this prelude's to claim, as a pseudo-argument's `(` claims its
        // list's lead: the list's first selector is not told the name bounds it, so its own
        // backward scan would reach into the NAME, which a run glued to it is part of, and
        // print that run a second time (`:--x<NBSP> a` → `:--x<NBSP> <NBSP> a`, growing every
        // pass). So the anchor stands down whatever the gap holds.
        let anchor = first_anchor(list);
        self.claimed_lead.set(Some(anchor));
        let kept = self.spell_gap_items(name.end, anchor, Edge::Presence, Edge::Presence);
        if kept.is_empty() {
            let gap = self.comment_blocks_in_range(name.end, list.span.start);
            if !gap.is_empty() {
                parts.push(d.text(" "));
                parts.push(d.text_pooled(&gap));
            }
            parts.push(d.line());
        } else if let Some(before_list) = kept.strip_suffix(' ') {
            // A run in the gap is spelled with the author's separation on both sides, and the
            // break the group may take is one of the spaces that separation wrote — the one
            // before the list where there is one, so the gap stays on the name's line, as its
            // comments do; the one after the name otherwise (`:--x <NBSP>a`, glued to the
            // list). A gap glued at both ends (`:--x<NBSP>/* c */<NBSP>a`) has no space to
            // break at, and is none of the group's.
            parts.push(d.text_pooled(before_list));
            parts.push(d.line());
        } else if let Some(after_name) = kept.strip_prefix(' ') {
            parts.push(d.line());
            parts.push(d.text_pooled(after_name));
        } else {
            parts.push(d.text_pooled(&kept));
        }
        parts.push(self.build_comma_list_doc(list.selectors, true));
        let doc = d.group(d.indent(d.concat(&parts)));

        // The terminator follows the group on its last line: `;` for the statement form,
        // ` {` for a block (not `@custom-selector` grammar, but accepted like any prelude).
        let suffix_width = if atrule.block.is_some() { 2 } else { 1 };
        if !claimed {
            self.write(" ");
        }
        self.write_arena_doc_with_suffix(doc, suffix_width);

        // Trailing gap: after the list, before the `;` / `{` (the prelude span's end).
        self.write_prelude_gap(list.span.end, span.end, Edge::Flush);
    }

    /// Emit a structural at-rule prelude gap `[start, end)` — the out-of-paren `@scope` gaps
    /// (leading / between the clauses / after `to` / pre-`{`), the `@custom-selector`
    /// leading / trailing gaps, and an `@import` prelude's tail before its `;` — and report
    /// whether a boundary claim printed it.
    ///
    /// Where the gap holds a boundary MEMBER the parser stepped (`@scope <NBSP> (.a)`,
    /// `(.a) to <NBSP> (.b)`), the gap is claimed whole, its comments in place
    /// ([`Self::spell_gap_items`]): the author's separation kept against the token before it
    /// (spaced from a name or a `)`, glued where they glued it — `to<NBSP>`), and on its far
    /// side as `trail` says — [`Edge::Presence`] ahead of a `(` or `to`, whose literal then
    /// writes no space of its own (`true`), [`Edge::Flush`] ahead of the `;` / `{`. Prettier
    /// drops the run at some of these and keeps it at others; tsv keeps it at all of them.
    ///
    /// Otherwise the gap's block comments print as ` /* … */` — a single leading space, then
    /// the comment(s) joined single-spaced (`comment_blocks_in_range`); prettier preserves
    /// the comment with the source spacing (`@scope`) or drops it (`@custom-selector`), tsv
    /// normalizes to single spaces. A gap with no comment writes nothing — the neighboring
    /// ` (`/` to`/` {` literals already carry the separator.
    fn write_prelude_gap(&mut self, start: u32, end: u32, trail: Edge) -> bool {
        if self.gap_holds_member(start, end) {
            let kept = self.spell_gap_items(start, end, Edge::Presence, trail);
            self.write(&kept);
            return true;
        }
        let text = self.comment_blocks_in_range(start, end);
        if !text.is_empty() {
            self.write(" ");
            self.write(&text);
        }
        false
    }

    /// Emit one `@scope` clause — ` (<selector-list>)` — interleaving any comment inside
    /// the parens (leading/trailing the list) via the clause's `paren` span, the same
    /// wrapping the `:is()` args use. The printer twin of the parser's `parse_scope_clause`.
    /// `spaced` is false where a boundary claim before it already spelled the author's
    /// separation (`write_prelude_gap`).
    fn write_scope_clause(&mut self, clause: &internal::ScopeClause<'_>, spaced: bool) {
        self.write(if spaced { " (" } else { "(" });
        self.print_selector_list_nested(&clause.list, Some(clause.paren));
        self.write(")");
    }

    /// Format an `@import` media query (doc-first).
    ///
    /// Prettier value-parses `@import` preludes and emits the media condition as
    /// `group(indent(fill(...)))`; the trailing `;` is the only suffix (1).
    ///
    /// A comment-bearing condition or a single query takes the doc-first `and`/`or`
    /// wrap (a whitespace fill would shatter `/* … */` comments). A comma-separated
    /// query *list* is the one prelude that stays imperative
    /// (`print_import_media_query_fill`): its two-level greedy fill keeps the first
    /// query on the `@import` line and breaks it *internally*, but tsv's renderer
    /// fill moves an over-wide first item to its own line (the load-bearing
    /// `at_line_start` divergence kept for Svelte), so nested doc fills can't
    /// reproduce prettier's layout. Confirmed: the list does **not** map to `fill()`.
    fn print_import_media_query(&mut self, content: &str) {
        // ⚠️ An `@import` prelude is a media query by *grammar*, but prettier does not read
        // it with the media-query reader: `isModuleRuleName` routes it to `parseValue`, the
        // same reader `@supports` takes. So it normalizes on the value path — no known-unit
        // gate, hex folded, a word absorbing an abutting number — which is the same split
        // that already leaves its media-feature *names* unfolded here.
        let normalized = value_normalization::normalize_value_text(content, ValueReader::Value);
        let MediaQueryList {
            text,
            entries: queries,
            closing_comma,
        } = media_query_list(&normalized);
        if text.contains("/*") || queries.len() <= 1 {
            // `text` already has a deletable closing comma removed, and keeps one the
            // entries need (`@import url('a.css'),;`).
            let doc = self.build_and_or_wrap_doc(text, 1);
            self.write_arena_doc_with_suffix(doc, 1);
        } else {
            self.print_import_media_query_fill(&queries, closing_comma);
        }
    }

    /// Greedy two-level fill for a comma-separated `@import` media-query list
    /// (imperative — see `print_import_media_query` for why it can't be a doc fill).
    ///
    /// Mirrors prettier's nested `group(indent(fill(...)))` — an outer fill over the
    /// comma-separated queries, each query an inner fill over its space-separated
    /// tokens:
    /// - queries pack greedily; a comma break indents one level (`+1`);
    /// - a query that overflows its line breaks at its `and`/space boundaries two
    ///   levels in (`+2`);
    /// - prettier's outer fill measures each query's *full flat width*, so an
    ///   overflowing query forces its trailing comma onto the next line while
    ///   consecutive short queries keep packing.
    ///
    /// (`split_by_space_preserving_parens` keeps the tokens atomic — no token has an
    /// internal break — so naive greedy line-packing is equivalent to prettier's
    /// pairwise `fill`.) The trailing `;`/`,` (1 wide) rides each query's last line.
    fn print_import_media_query_fill(&mut self, queries: &[&str], closing_comma: bool) {
        let base = self.effective_indent();
        let indent1 = (base + 1) * TAB_WIDTH; // comma-break column
        let indent2 = (base + 2) * TAB_WIDTH; // within-query break column

        let mut col = self.current_column();
        let n = queries.len();
        for (qi, query) in queries.iter().enumerate() {
            let is_last = qi == n - 1;
            let query_start = col;
            col = self.emit_import_query(query, query_start, indent2, PRINT_WIDTH);
            if is_last {
                // A list-closing comma takes no separator after it — one would strand
                // a space before the rule's `;`.
                if closing_comma {
                    self.write(",");
                }
                continue;
            }
            self.write(",");
            col += 1;
            // Outer-fill separator: prettier glues each comma to its query and
            // measures `[query_i ",", " ", query_{i+1} ","]` from this query's start
            // (the trailing comma counts; the final `;` lives outside the fill, so the
            // *last* query carries no comma here). A query whose flat width overflowed
            // forces the comma to break; otherwise the next query packs inline if it fits.
            let next = queries[qi + 1];
            let next_comma = usize::from(qi + 1 != n - 1);
            if query_start + query.len() + next.len() + 2 + next_comma <= PRINT_WIDTH {
                self.write(" ");
                col += 1;
            } else {
                self.write("\n");
                self.write_indent_extra(1);
                col = indent1;
            }
        }
    }

    /// Emit one media query, greedy-filling its space-separated tokens; internal
    /// breaks land at `indent2`. Returns the ending visual column.
    fn emit_import_query(
        &mut self,
        query: &str,
        start_col: usize,
        indent2: usize,
        width: usize,
    ) -> usize {
        let atoms = value_normalization::split_by_space_preserving_parens(query);
        let last = atoms.len().saturating_sub(1);
        let mut col = start_col;
        for (ai, &atom) in atoms.iter().enumerate() {
            // Connector case is preserved (matching prettier); emit `atom` verbatim.
            if ai == 0 {
                self.write(atom);
                col += atom.len();
                continue;
            }
            // The final token carries the trailing `,`/`;` (1 wide); reserve for it.
            let reserve = usize::from(ai == last);
            if col + 1 + atom.len() + reserve <= width {
                self.write(" ");
                self.write(atom);
                col += 1 + atom.len();
            } else {
                self.write("\n");
                self.write_indent_extra(2);
                self.write(atom);
                col = indent2 + atom.len();
            }
        }
        col
    }
}
