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
use super::value_normalization;
use super::value_normalization::ValueReader;
use crate::ast::internal;
use tsv_lang::Span;
use tsv_lang::doc::{DocBuf, DocContext, arena::DocId};
use tsv_lang::source_scan;
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
    let entries: Vec<&str> = parts.into_iter().map(str::trim).collect();
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
                    // The media condition of an `@import` prelude — its last value, and the
                    // only one that is a bare identifier run rather than a string/function.
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
                // `@import 'a.css' /* c */;`).
                for comment in self.comments_to_emit_between(prev_end, atrule.span.end) {
                    self.write(" ");
                    self.print_css_comment(comment);
                }
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
            internal::PreludeValue::Selectors { root, limit, .. } => {
                // @scope selector lists: `@scope [(root)]? [to (limit)]?`. Both clauses
                // are independently optional (css-cascade-6), so a bare `@scope { … }`
                // writes no prelude and `@scope to (limit)` writes only the limit.
                // These are nested context, so they don't wrap (same as :is(), :where()).
                //
                // Each clause's `paren` span recovers a comment leading/trailing the list
                // *inside* the parens (the same wrapping the `:is()` args use). The
                // out-of-paren prelude gaps — leading (`@scope /* c */ (.a)`), between the
                // root `)` and `to`, between `to` and the limit `(`, and after the last `)`
                // before the block `{` — re-emit their comments here too, normalized to a
                // single space on each side (prettier freezes the source spacing; a
                // cataloged divergence — see conformance_prettier_css.md §CSS: Comments).
                //
                // Right bound of the pre-`{` gap. A block-less `@scope` isn't valid CSS,
                // but fall back to the rule's `;` end so the range stays well-formed.
                let block_start = atrule
                    .block
                    .as_ref()
                    .map_or(atrule.span.end, |b| b.span.start);

                // Leading gap: the first structural token after `@scope` is the root `(`,
                // else `to`, else the block `{`. Its left bound is the `@` — no comment can
                // sit inside the `@scope` at-keyword token, so it never double-counts an
                // in-paren comment.
                let first_start = root
                    .as_ref()
                    .map(|r| r.paren.start)
                    .or_else(|| limit.as_ref().map(|l| l.to_span.start))
                    .unwrap_or(block_start);
                self.write_scope_gap_comments(atrule.span.start, first_start);

                if let Some(root) = root {
                    self.write_scope_clause(root);
                }
                if let Some(limit) = limit {
                    // Between-clause gap: root `)` → `to` (only when a root precedes it).
                    if let Some(root) = root {
                        self.write_scope_gap_comments(root.paren.end, limit.to_span.start);
                    }
                    self.write(" to");
                    // After-`to` gap: `to` → limit `(`.
                    self.write_scope_gap_comments(limit.to_span.end, limit.clause.paren.start);
                    self.write_scope_clause(&limit.clause);
                }
                // Pre-`{` gap: after the last clause's `)` (only when a clause exists — a
                // bare `@scope /* c */ {` comment is the leading gap above).
                if let Some(last_end) = limit
                    .as_ref()
                    .map(|l| l.clause.paren.end)
                    .or_else(|| root.as_ref().map(|r| r.paren.end))
                {
                    self.write_scope_gap_comments(last_end, block_start);
                }
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
            self.print_css_block_children(block.children, 0);
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
        let fill = d.with_context(fill, DocContext::reserving(suffix_width));
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
        self.print_condition_query(kind, condition, atrule.block.is_some(), Some(span));
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
        has_block: bool,
        prelude_span: Option<Span>,
    ) {
        // The partless prelude is `write_condition_prelude`'s, this function's one caller
        // — it owns the name→prelude separator too, for the same reason. Everything below
        // may therefore assume a part exists.
        debug_assert!(
            !condition.parts.is_empty(),
            "a partless condition prelude is write_condition_prelude's to print"
        );
        // Print optional name prefix (for @container)
        let name_end_pos = if let Some(n) = kind.name() {
            self.write(n);
            // Separate the name from its condition.
            self.write(" ");
            // Find where the name ends in source (name length from prelude start)
            prelude_span.map(|s| s.start + n.len() as u32)
        } else {
            prelude_span.map(|s| s.start)
        };

        let suffix_width = if has_block { " {".len() } else { 0 };
        let doc = self.build_condition_query_doc(
            kind,
            condition,
            name_end_pos,
            prelude_span,
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
            // rule's selector list; `breakable = false` joins with a literal space, and
            // `remove_lines` keeps the argument on the prelude's line either way.
            internal::ConditionSegment::Selectors(selectors) => {
                d.remove_lines(self.build_comma_list_doc(selectors, false))
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
        name_end_pos: Option<u32>,
        prelude_span: Option<Span>,
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

        // The gap between the optional name and the first part: its comments, the first
        // part's own connector run, and the boundary run
        // (`Self::condition_part_head_ws`), which lands flush against the part after them
        // all, like every other claim.
        //
        // ⚠️ That gap can hold a **connector run** — the first part's own, since a
        // connector with no operand on its LEFT still binds the part on its right
        // (`@supports and (a: b)`, `@container name and (a: b)`). It is emitted here for
        // the same reason the separator below emits an interior one, and the gap's
        // comments split around it the same way: the whole question is one question, and
        // an arm that answers only "which comments" drops the run's words silently.
        let leading_first = |part: &internal::ConditionPart<'_>| -> DocId {
            let (before, after) = name_end_pos
                .map(|start| {
                    self.extract_comments_split_by_connector(start, part.span.start, part.connector)
                })
                .unwrap_or_default();
            let kept = name_end_pos
                .map(|start| self.condition_part_head_ws(start, part))
                .unwrap_or_default();
            let mut head = DocBuf::new();
            for piece in [
                before.as_str(),
                part.connector_run.unwrap_or_default(),
                after.as_str(),
            ] {
                if piece.is_empty() {
                    continue;
                }
                if !head.is_empty() {
                    head.push(d.text(" "));
                }
                head.push(d.text_pooled(piece));
            }
            if head.is_empty() {
                if kept.is_empty() {
                    return content_doc(part);
                }
                return d.concat(&[d.text_pooled(&kept), content_doc(part)]);
            }
            // ⚠️ The space is the NAME SEPARATOR, not decoration: a connector is an
            // identifier, and `read_identifier` takes every code point at or above U+00A0 as
            // content, so a boundary run emitted flush after one re-parses as the single name
            // `and<NBSP>` — a prelude that then falls to the raw path, its condition
            // unreadable, and a fixed point either way. `printer/boundary_ws.rs` §Where a
            // claim is emitted states the rule for the family this juncture joined.
            head.push(d.text(" "));
            if !kept.is_empty() {
                head.push(d.text_pooled(&kept));
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
            return match self.condition_tail_doc(first, prelude_span, condition) {
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
            // Separator: ` <before-comments>? <connector> line`. The connector and any
            // pre-connector comment stay on the previous line; the `line` breaks before
            // the content. Post-connector comments lead the content on the next line.
            let (before, after) = self.extract_comments_split_by_connector(
                parts[i - 1].span.end,
                part.span.start,
                part.connector,
            );
            let mut sep = DocBuf::new();
            if !before.is_empty() {
                sep.push(d.text(" "));
                sep.push(d.text_pooled(&before));
            }
            // Emit the connector run's source text (`AND` stays `AND`), preserved like
            // prettier. `connector_run` is `Some` whenever `connector` is.
            if let Some(conn_run) = part.connector_run {
                sep.push(d.text(" "));
                sep.push(d.text_pooled(conn_run));
            }
            sep.push(d.line());
            fill_parts.push(d.concat(&sep));

            let mut chunk = DocBuf::new();
            if !after.is_empty() {
                chunk.push(d.text_pooled(&after));
                chunk.push(d.text(" "));
            }
            // This part's own head run, exactly as `leading_first` claims the first part's —
            // bounded at the part's span so it can never reach back over the connector, whose
            // side of the gap rides out in the separator above.
            let kept = self.condition_part_head_ws(part.span.start, part);
            if !kept.is_empty() {
                chunk.push(d.text_pooled(&kept));
            }
            chunk.push(content_doc(part));
            fill_parts.push(d.concat(&chunk));
        }

        // The query's tail rides the last part's line, exactly as it rides the only part's
        // in the single-part arm above.
        if let Some(tail) = parts
            .last()
            .and_then(|last| self.condition_tail_doc(last, prelude_span, condition))
            && let Some(last_chunk) = fill_parts.pop()
        {
            fill_parts.push(d.concat(&[last_chunk, tail]));
        }

        let fill = d.fill(&fill_parts);
        let fill = d.with_context(fill, DocContext::reserving(suffix_width));
        d.indent(fill)
    }

    /// The query's own tail, to append to whatever printed its LAST part: the comments
    /// standing between that part and the end of the prelude, then the run of operators with
    /// no operand ([`Self::trailing_operators_doc`]).
    ///
    /// One emitter because [`Self::build_condition_query_doc`]'s two arms ask the same two
    /// questions of the same stretch, and an arm that answers one of them its own way is
    /// exactly the drift this prelude has already paid for once — the single-part arm emitted
    /// no connector at all while its twin did. The two arms differ in where the tail LANDS (a
    /// concat, or the fill's last chunk), which is all they should differ in.
    ///
    /// `None` when the query has neither, so a caller need not concat an empty doc.
    fn condition_tail_doc(
        &self,
        last: &internal::ConditionPart<'_>,
        prelude_span: Option<Span>,
        condition: &internal::ConditionQuery<'_>,
    ) -> Option<DocId> {
        let d = self.d();
        let mut tail = DocBuf::new();
        if let Some(span) = prelude_span {
            let trailing = self.comment_blocks_in_range(last.span.end, span.end);
            if !trailing.is_empty() {
                tail.push(d.text(" "));
                tail.push(d.text_pooled(&trailing));
            }
        }
        if let Some(operators) = self.trailing_operators_doc(condition) {
            tail.push(operators);
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
    /// It can never collide with the trailing-comment claim beside it. A comment inside
    /// the run is carried *by* the run (the parser un-registers it there), so whenever
    /// this is `Some`, the `comment_blocks_in_range` sweep over the same stretch found
    /// nothing.
    fn trailing_operators_doc(&self, condition: &internal::ConditionQuery<'_>) -> Option<DocId> {
        let trailing_operators = condition.trailing_operators?;
        let d = self.d();
        Some(d.concat(&[d.text(" "), d.text_pooled(trailing_operators)]))
    }

    /// The boundary-whitespace run at a condition part's head — the ONE gap of a condition
    /// prelude the printer regenerates rather than carrying inside a part's own text
    /// (`@supports <NBSP>(a: b)`, `@container name <NBSP>(…)`), so the ONE gap that owes a
    /// claim (`printer/boundary_ws.rs` §Who claims where).
    ///
    /// The sweep reaches PAST the part's own span start: a condition part opens on whatever
    /// token the lexer produced, and a boundary run at the head of the prelude IS that token
    /// (`@supports <NBSP>(a: b)` opens the part on the run, not on the `(`). So it runs from
    /// `gap_start` to the part's first real byte, which `skip_gap_trivia` finds with the
    /// boundary class the parser skipped by — one range covering both the gap before the part
    /// and the trivia inside it.
    ///
    /// `gap_start` is the caller's, and the two callers differ deliberately: the first part
    /// sweeps from the name's end (there being no previous part), every later one from its own
    /// span start, so it can never reach back over the connector whose side of the gap rides
    /// out in the separator.
    fn condition_part_head_ws(&self, gap_start: u32, part: &internal::ConditionPart<'_>) -> String {
        self.boundary_ws_in_gap(
            gap_start,
            super::boundary_ws::skip_gap_trivia(self.source, part.span.start, part.span.end),
        )
    }

    /// Extract comments from a source range, split around the connector keyword.
    ///
    /// Returns (comments_before_connector, comments_after_connector); for
    /// `/* a */ and /* b */` → (`/* a */`, `/* b */`). The connector is located
    /// comment-aware via `find_keyword_ascii_case_insensitive` (CSS trivia profile),
    /// so a `and`/`or` buried in a comment (`/* x and y */ and …`) doesn't move the
    /// split into the comment — which would drop it (a straddling comment is in
    /// neither half-range). The match is ASCII case-insensitive because the parser
    /// accepts uppercase connectors (`AND`/`Or`), which CSS Syntax 3 makes valid.
    /// With no connector (or none found) the whole run goes before. Delegates the
    /// binning + join to the shared `split_comments_at`.
    ///
    /// ⚠️ The gap may hold a **run** of operators (`(a: b) and or (c: d)`), and this
    /// scans for the *first* spelling of `connector` — the run's LAST kind — so the
    /// position it splits at can be any operator of the run, not reliably the last.
    /// It bins every registered comment correctly all the same, and the reason is the
    /// parser's, not this scan's: a comment *inside* the run rides
    /// `ConditionPart::connector_run`'s own text and gives up its registration, so the
    /// only comments left to bin sit before the run's first word or after its last —
    /// on whichever side of the split point that puts them, which for those two is the
    /// same side either way. Don't "improve" this into a last-occurrence scan without
    /// that fact: it is what makes the whole family correct, not the choice of
    /// occurrence.
    fn extract_comments_split_by_connector(
        &self,
        start: u32,
        end: u32,
        connector: Option<internal::ConditionConnector>,
    ) -> (String, String) {
        let connector_keyword = match connector {
            Some(internal::ConditionConnector::And) => "and",
            Some(internal::ConditionConnector::Or) => "or",
            None => return self.split_comments_at(start, end, None),
        };

        let connector_pos = source_scan::find_keyword_ascii_case_insensitive(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            connector_keyword.as_bytes(),
            source_scan::TriviaProfile::CSS,
        )
        .map(|pos| pos as u32);

        self.split_comments_at(start, end, connector_pos)
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
    fn write_import_gap_comments(
        &mut self,
        start: u32,
        end: u32,
        needs_separator: bool,
        glue_next: bool,
    ) {
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

    /// Emit any block comments in `[start, end]` as ` /* … */` — a single leading
    /// space, then the comment(s) joined single-spaced (`comment_blocks_in_range`).
    ///
    /// The out-of-paren `@scope` prelude gaps (leading / between the clauses / after
    /// `to` / pre-`{`) call this at each authored position; prettier preserves the
    /// comment with the source spacing, tsv normalizes to single spaces. A gap with no
    /// comment writes nothing — the neighboring ` (`/` to`/` {` literals already carry
    /// the separator.
    fn write_scope_gap_comments(&mut self, start: u32, end: u32) {
        let text = self.comment_blocks_in_range(start, end);
        if !text.is_empty() {
            self.write(" ");
            self.write(&text);
        }
    }

    /// Emit one `@scope` clause — ` (<selector-list>)` — interleaving any comment inside
    /// the parens (leading/trailing the list) via the clause's `paren` span, the same
    /// wrapping the `:is()` args use. The printer twin of the parser's `parse_scope_clause`.
    fn write_scope_clause(&mut self, clause: &internal::ScopeClause<'_>) {
        self.write(" (");
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
