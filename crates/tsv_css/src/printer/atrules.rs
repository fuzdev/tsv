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
use crate::ast::internal;
use tsv_lang::Span;
use tsv_lang::comments_in_range;
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
        self.write("@");
        // At-rule names are ASCII case-insensitive; lowercase for output (`@MEDIA`
        // → `@media`), matching prettier. The stored `name` keeps its source case
        // (public AST matches Svelte).
        self.write(&value_normalization::lowercase_at_rule_name(atrule.name));

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
                    self.write_import_gap_comments(prev_end, value.span().start, i > 0);
                    // Check if this is the media query part of @import that needs wrapping
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
                        // Route a wrappable media condition through the line-wrapping
                        // path: an `and`/`or`-joined query, or a comma-separated query
                        // *list* (prettier value-parses `@import` and fills the whole
                        // list). A comma-only list with comments stays on the
                        // comment-aware value path below — the fill splits on whitespace
                        // and would shatter `/* … */` comments (the `and`/`or` path keeps
                        // its existing comment handling).
                        // Connector detection is ASCII case-insensitive — `AND`/`Or`
                        // are valid connectors (CSS Syntax 3) that route to the
                        // wrapping path (which preserves their source case). Only fold
                        // case when there's uppercase to fold; the common all-lowercase
                        // query probes the original directly (no allocation).
                        let lower = if normalized.bytes().any(|b| b.is_ascii_uppercase()) {
                            Cow::Owned(normalized.to_ascii_lowercase())
                        } else {
                            Cow::Borrowed(&*normalized)
                        };
                        if lower.contains(" and ")
                            || lower.contains(" or ")
                            || (normalized.contains(',') && !normalized.contains("/*"))
                        {
                            self.print_import_media_query(&normalized);
                            prev_end = value.span().end;
                            continue;
                        }
                    }
                    // Use doc-based formatting to normalize quotes and spacing
                    self.print_nested_value(value);
                    prev_end = value.span().end;
                }
                // Trailing comments between the last value and the `;` (e.g.
                // `@import 'a.css' /* c */;`).
                for comment in comments_in_range(self.comments, prev_end, atrule.span.end) {
                    self.write(" ");
                    self.print_css_comment(comment);
                }
            }
            internal::PreludeValue::Raw { content, .. } if !content.is_empty() => {
                // `content` is already verbatim (internal whitespace + comments preserved,
                // outer-trimmed, `url()` inner-trimmed) from the parser's non-normalized
                // raw path, so it matches prettier as-is — no comment-spacing rewrite.
                // Embedded newlines survive under Svelte `<style>` because the CSS renders
                // at its final indent (no post-hoc line re-indent to compound them).
                self.write(" ");
                self.write(content);
            }
            internal::PreludeValue::Supports { condition, span } => {
                self.write(" ");
                // @supports conditions are declarations, so prettier normalizes
                // their values (e.g. numbers); @container queries are left raw.
                self.print_condition_query(
                    ConditionKind::Supports,
                    condition,
                    atrule.block.is_some(),
                    Some(*span),
                );
            }
            internal::PreludeValue::Container {
                name,
                condition,
                span,
            } => {
                self.write(" ");
                self.print_condition_query(
                    ConditionKind::Container {
                        name: name.as_deref(),
                    },
                    condition,
                    atrule.block.is_some(),
                    Some(*span),
                );
            }
            internal::PreludeValue::Media { content, .. } => {
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
                // cataloged divergence — see conformance_prettier.md §CSS: Comments).
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

    /// Build the doc tree for an `@media` prelude (see `print_media_prelude`).
    fn build_media_prelude_doc(&self, content: &str, suffix_width: usize) -> DocId {
        let d = self.d();
        // Normalize numbers + string quotes in the raw prelude (`.5px` → `0.5px`,
        // `"x"` → `'x'`), matching the declaration-value path. Comments preserved.
        // Hex case is preserved (`lowercase_hex` off) — prettier only lowercases hex
        // in `@supports` condition declarations, not `@media` feature values.
        let content = value_normalization::normalize_value_text(content, false);
        // Lowercase media-feature *names* (`(MIN-WIDTH: …)` → `(min-width: …)`),
        // matching prettier; media types, `and`/`or`/`not`/`only`, and feature values
        // are preserved (see `lowercase_media_feature_names`). Keep the already-owned
        // `content` when nothing changed (the common no-uppercase case borrows).
        let content = match value_normalization::lowercase_media_feature_names(&content) {
            Cow::Borrowed(_) => content,
            Cow::Owned(s) => s,
        };

        let queries: Vec<&str> = value_normalization::split_args_by_comma(&content)
            .into_iter()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .collect();

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
            return d.group(d.indent(d.concat(&parts)));
        }

        // Single query: wrap greedily at its `and`/`or` boundaries.
        self.build_and_or_wrap_doc(&content, suffix_width)
    }

    /// Build an `and`/`or`-wrapping fill for a single media query (raw string).
    ///
    /// Splits the query into segments at its top-level `and`/`or` keywords (paren-,
    /// quote- and comment-aware via `split_by_space_preserving_parens`) and joins
    /// them with breakable separators that keep the keyword on the line before the
    /// break (`… and⏎\t…`). A query with no top-level `and`/`or` has no break point,
    /// so it's emitted verbatim. Shared by `@media` and `@import` single queries.
    fn build_and_or_wrap_doc(&self, query: &str, suffix_width: usize) -> DocId {
        let d = self.d();
        let atoms = value_normalization::split_by_space_preserving_parens(query);

        // Re-group atoms into segments split at `and`/`or` keyword atoms. The
        // connector rides the separator before the following segment.
        let mut fill_parts = DocBuf::new();
        let mut segment = d.pool_writer();
        let mut has_connector = false;
        for atom in atoms {
            // A `and`/`or` connector (case-insensitive) is the wrap break point;
            // its case is preserved (emit the original `atom`).
            if is_media_connector(atom) && !segment.is_empty() {
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
        let fill = d.with_context(
            fill,
            DocContext {
                trailing_reserve: suffix_width,
                ..Default::default()
            },
        );
        d.indent(fill)
    }

    /// Format an `@supports`/`@container` condition prelude (doc-first).
    ///
    /// The whole prelude is one doc tree rendered through the renderer, so the
    /// wrap decision and emission share a single representation (no measure pass to
    /// drift from emission). The condition parts join into a `fill` whose `line`s
    /// break at the `and`/`or` boundaries — the keyword stays on line 1, the
    /// condition wraps to line 2 — with the trailing ` {` reserved so the boundary
    /// breaks at print width. tsv wraps where prettier never does (a deliberate,
    /// cataloged divergence — see conformance_prettier.md §CSS: At-Rules):
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
        // Print optional name prefix (for @container)
        let name_end_pos = if let Some(n) = kind.name() {
            self.write(n);
            // Separate the name from its condition with a space — but a name with no
            // condition (`@container b {`) takes none, else the block's ` {` would
            // stack into a double space.
            if !condition.parts.is_empty() {
                self.write(" ");
            }
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

    /// Build the doc tree for an `@supports`/`@container` condition prelude.
    ///
    /// A single condition has no break point — it's a plain concat (leading comment,
    /// content, then trailing comment), emitted inline like prettier. Two or more
    /// conditions become `indent(fill([...]))`: the connector (`and`/`or`) and any
    /// comments split around it ride a breakable separator (the connector stays on
    /// the line before the break), and the trailing ` {`/`;` is reserved via the
    /// fill's `trailing_reserve` so the boundary breaks at print width.
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

        // `@supports` values are number-normalized (`.5px` → `0.5px`) and their hex
        // colors lowercased (`#FFF` → `#fff`); `@container` is emitted verbatim, both
        // matching prettier.
        let content_doc = |part: &internal::ConditionPart<'_>| {
            if kind.normalizes() {
                // Only `@supports` reaches here (the sole `normalizes()` kind), so hex
                // lowercasing is on.
                d.text_pooled(&value_normalization::normalize_value_text(
                    part.content,
                    true,
                ))
            } else {
                d.text_pooled(part.content)
            }
        };

        // The leading comments before the first part (after the optional name).
        let leading_first = |part: &internal::ConditionPart<'_>| -> DocId {
            let leading = name_end_pos
                .map(|start| self.comment_blocks_in_range(start, part.span.start))
                .unwrap_or_default();
            if leading.is_empty() {
                content_doc(part)
            } else {
                d.concat(&[d.text_pooled(&leading), d.text(" "), content_doc(part)])
            }
        };

        if parts.len() <= 1 {
            // Single condition: no break point, emit inline (leading + content + trailing).
            let Some(first) = parts.first() else {
                return d.text("");
            };
            let mut chunk = DocBuf::new();
            chunk.push(leading_first(first));
            if let Some(span) = prelude_span {
                let trailing = self.comment_blocks_in_range(first.span.end, span.end);
                if !trailing.is_empty() {
                    chunk.push(d.text(" "));
                    chunk.push(d.text_pooled(&trailing));
                }
            }
            return d.concat(&chunk);
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
            // Emit the connector's source case (`AND` stays `AND`), preserved like
            // prettier. `connector_raw` is `Some` whenever `connector` is.
            if let Some(conn_raw) = part.connector_raw {
                sep.push(d.text(" "));
                sep.push(d.text_pooled(conn_raw));
            }
            sep.push(d.line());
            fill_parts.push(d.concat(&sep));

            let mut chunk = DocBuf::new();
            if !after.is_empty() {
                chunk.push(d.text_pooled(&after));
                chunk.push(d.text(" "));
            }
            chunk.push(content_doc(part));
            fill_parts.push(d.concat(&chunk));
        }

        // Trailing comments after the last part ride its line.
        if let (Some(last), Some(span)) = (parts.last(), prelude_span) {
            let trailing = self.comment_blocks_in_range(last.span.end, span.end);
            if !trailing.is_empty()
                && let Some(last_chunk) = fill_parts.pop()
            {
                fill_parts.push(d.concat(&[last_chunk, d.text(" "), d.text_pooled(&trailing)]));
            }
        }

        let fill = d.fill(&fill_parts);
        let fill = d.with_context(
            fill,
            DocContext {
                trailing_reserve: suffix_width,
                ..Default::default()
            },
        );
        d.indent(fill)
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
    fn write_import_gap_comments(&mut self, start: u32, end: u32, needs_separator: bool) {
        let comments: Vec<_> = comments_in_range(self.comments, start, end).collect();
        if comments.is_empty() {
            if needs_separator {
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
        self.write(" ");
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
        // `@import`'s prelude is a media query (feature values), so hex case is
        // preserved like `@media` — only `@supports` conditions lowercase hex.
        let normalized = value_normalization::normalize_value_text(content, false);
        let queries: Vec<&str> = value_normalization::split_args_by_comma(&normalized)
            .into_iter()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .collect();
        if normalized.contains("/*") || queries.len() <= 1 {
            let doc = self.build_and_or_wrap_doc(&normalized, 1);
            self.write_arena_doc_with_suffix(doc, 1);
        } else {
            self.print_import_media_query_fill(&queries);
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
    fn print_import_media_query_fill(&mut self, queries: &[&str]) {
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
