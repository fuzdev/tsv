// CSS at-rule formatting
//
// Handles formatting of:
// - At-rules (@media, @keyframes, @supports, @import, @layer, @font-face, etc.)
// - At-rule blocks and their children (rules, declarations, nested at-rules)
//
// ## Architecture
//
// This module uses doc builders for width-based decisions (e.g., condition query
// wrapping). The complex prelude and block handling remains imperative for clarity.

use std::borrow::Cow;

use super::Printer;
use super::value_normalization;
use crate::ast::internal;
use tsv_lang::doc::{self, DocBuf, Mode, arena::DocId};
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};
use tsv_lang::{comments_in_range, is_format_ignore_directive};

/// A condition-query part with its content prepared for printing.
///
/// `@supports` parts are value-normalized into a fresh `String`; `@container`
/// parts print verbatim, so they borrow the AST's `&'arena str`
/// (`ConditionPart::content`) directly. The `Cow` carries either form without
/// forcing an allocation on the verbatim path.
struct NormalizedConditionPart<'c> {
    connector: Option<internal::ConditionConnector>,
    content: Cow<'c, str>,
    span: tsv_lang::Span,
}

/// Convert a condition connector to its string representation
fn connector_str(conn: internal::ConditionConnector) -> &'static str {
    match conn {
        internal::ConditionConnector::And => "and",
        internal::ConditionConnector::Or => "or",
    }
}

/// How a media-query prelude wraps when it exceeds print width.
#[derive(Clone, Copy)]
enum MediaWrap {
    /// `@media` — a comma-separated media-query list; break at every top-level
    /// comma (one query per line). A single `and`-joined query still falls back
    /// to `AndOr` wrapping.
    CommaList,
    /// `@import` media conditions — wrap only at the last fitting `and`/`or`.
    AndOr,
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
        self.write(atrule.name);

        // Print prelude based on type
        match &atrule.prelude {
            internal::PreludeValue::Values { values, span } if !values.is_empty() => {
                self.write(" ");
                // Special handling for @import with media query (last value may need wrapping)
                let is_import = atrule.name == "import";
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
                        if normalized.contains(" and ")
                            || normalized.contains(" or ")
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
                // @scope selector lists: @scope (root) to (limit)
                // These are nested context, so they don't wrap (same as :is(), :where())
                self.write(" (");
                self.print_selector_list_nested(root);
                self.write(")");
                if let Some(limit_selectors) = limit {
                    self.write(" to (");
                    self.print_selector_list_nested(limit_selectors);
                    self.write(")");
                }
            }
            _ => {}
        }

        if let Some(block) = &atrule.block {
            self.write(" {\n");
            self.indent_level += 1;

            let mut i = 0;
            let mut format_ignore_next = false;
            while i < block.children.len() {
                let child = &block.children[i];

                // For non-first children, add newline before rules/at-rules/comments
                if i > 0 {
                    match child {
                        internal::CssBlockChild::Declaration(_) => {
                            // Preserve blank line between consecutive declarations
                            if self.has_blank_line_before_child(block.children, i) {
                                self.write("\n");
                            }
                        }
                        internal::CssBlockChild::Rule(_)
                        | internal::CssBlockChild::Atrule(_)
                        | internal::CssBlockChild::Comment(_) => {
                            let has_blank_line =
                                self.has_blank_line_before_child(block.children, i);

                            // Declarations end with \n, standalone comments end with \n,
                            // but rules/at-rules end with } and inline comments end with */
                            // Check the actual output buffer rather than child type
                            if !self.output_ends_with_newline() {
                                self.write("\n"); // Separator
                            }
                            if has_blank_line {
                                self.write("\n"); // Blank line
                            }
                        }
                    }
                }

                // Format the child with appropriate indentation handling
                match child {
                    internal::CssBlockChild::Declaration(_) => {
                        // Declaration will write its own indentation
                        self.print_atrule_block_child(child);
                    }
                    internal::CssBlockChild::Rule(_) | internal::CssBlockChild::Atrule(_) => {
                        // Rules and at-rules need indentation
                        self.write_indent();
                        if format_ignore_next {
                            self.write(child.span().extract(self.source));
                            format_ignore_next = false;
                        } else {
                            self.print_atrule_block_child(child);
                        }

                        // Check if next child is an inline comment
                        let inline_count =
                            self.try_print_inline_comments(block.children, i, child.span().end);
                        i += inline_count;
                    }
                    internal::CssBlockChild::Comment(comment) => {
                        // Check for a format-ignore directive
                        if is_format_ignore_directive(comment.content(self.source)) {
                            format_ignore_next = true;
                        }
                        // Standalone comment
                        self.write_indent();
                        self.print_atrule_block_child(child);
                        self.write("\n");
                    }
                }

                i += 1;
            }

            self.indent_level -= 1;

            // Write the newline before `}` only when the last child didn't already
            // end one. Declarations end with `\n`; an empty block has no children at
            // all, so the `{\n` already opened the line — prettier renders empty
            // at-rule blocks as `{\n}` (no blank line inside).
            if !block.children.is_empty()
                && !matches!(
                    block.children.last(),
                    Some(internal::CssBlockChild::Declaration(_))
                )
            {
                self.write("\n");
            }
            self.write_indent();
            self.write("}");
        } else {
            self.write(";");
        }
    }

    /// Format an at-rule block child (rule, declaration, or nested at-rule)
    fn print_atrule_block_child(&mut self, child: &internal::CssBlockChild<'_>) {
        match child {
            internal::CssBlockChild::Rule(rule) => {
                // Format rule selector and opening brace
                self.print_selector_list(&rule.selector);

                // Check if first child is a comment after selector (before {)
                let mut start_index = 0;
                if let Some(internal::CssBlockChild::Comment(comment)) = rule.declarations.first() {
                    // Check if comment is on same line as selector AND before the opening brace
                    // If there's a '{' between selector and comment, the comment is inside the block, not after selector
                    if self.is_same_line(rule.selector.span.end, comment.span.start)
                        && !self
                            .has_opening_brace_between(rule.selector.span.end, comment.span.start)
                    {
                        // Print comment inline after selector
                        self.write(" /*");
                        self.write(comment.content(self.source));
                        self.write("*/");
                        start_index = 1; // Skip this comment when processing declarations
                    }
                }

                self.write(" {\n");

                // Format declarations and comments with proper indentation
                self.indent_level += 1;
                let mut i = start_index;
                let mut format_ignore_next = false;
                while i < rule.declarations.len() {
                    let block_child = &rule.declarations[i];
                    match block_child {
                        internal::CssBlockChild::Declaration(decl) => {
                            // Preserve blank line between consecutive declarations
                            if i > start_index
                                && self.has_blank_line_before_child(rule.declarations, i)
                            {
                                self.write("\n");
                            }
                            if format_ignore_next {
                                self.write_format_ignore_declaration(decl);
                                format_ignore_next = false;
                            } else {
                                self.print_css_declaration(decl);
                            }

                            // Check for inline comments after the declaration
                            let inline_count = self.try_print_inline_comments_after_decl(
                                rule.declarations,
                                i,
                                decl.span.end,
                            );
                            if inline_count > 0 {
                                i += inline_count;
                            }
                        }
                        internal::CssBlockChild::Comment(comment) => {
                            // Standalone comment (not inline after a declaration)
                            // Check if there's a blank line before this comment in source
                            if i > start_index
                                && self.has_blank_line_before_child(rule.declarations, i)
                            {
                                // Source has blank line - add it
                                // Note: Previous element already ended with \n, so one more \n gives blank line
                                self.write("\n");
                            }
                            // Check for a format-ignore directive
                            if is_format_ignore_directive(comment.content(self.source)) {
                                format_ignore_next = true;
                            }
                            self.write_indent();
                            self.print_css_comment(comment);
                            self.write("\n");
                        }
                        internal::CssBlockChild::Rule(nested_rule) => {
                            // CSS Nesting Module - format nested rule inside at-rule block rule
                            if i > start_index && !Self::prev_is_comment(rule.declarations, i) {
                                self.write("\n");
                            }
                            self.write_indent();
                            if format_ignore_next {
                                self.write(nested_rule.span.extract(self.source));
                                format_ignore_next = false;
                            } else {
                                self.print_css_rule(nested_rule);
                            }

                            // Check for inline comment after nested rule's closing brace
                            let inline_count = self.try_print_inline_comments(
                                rule.declarations,
                                i,
                                nested_rule.span.end,
                            );

                            self.write("\n");
                            i += inline_count;
                        }
                        internal::CssBlockChild::Atrule(nested_atrule) => {
                            // Nested at-rule inside rule
                            if i > start_index && !Self::prev_is_comment(rule.declarations, i) {
                                self.write("\n");
                            }
                            self.write_indent();
                            if format_ignore_next {
                                self.write(nested_atrule.span.extract(self.source));
                                format_ignore_next = false;
                            } else {
                                self.print_css_atrule(nested_atrule);
                            }
                            self.write("\n");
                        }
                    }
                    i += 1;
                }
                self.indent_level -= 1;

                // Closing brace at current indentation level (inside at-rule)
                self.write_indent();
                self.write("}");
            }
            internal::CssBlockChild::Declaration(decl) => self.print_css_declaration(decl),
            internal::CssBlockChild::Atrule(atrule) => self.print_css_atrule(atrule),
            internal::CssBlockChild::Comment(comment) => self.print_css_comment(comment),
        }
    }

    /// Format @media prelude with line-width wrapping at `and`/`or` boundaries
    ///
    /// Unlike @supports/@container, @media uses raw string parsing to preserve comments.
    /// Wrapping is done by finding `and`/`or` boundaries in the raw string.
    ///
    /// ```css
    /// @media screen and (min-width: 768px) and (max-width: 1024px) and
    ///     (orientation: landscape) {
    /// ```
    fn print_media_prelude(&mut self, content: &str, has_block: bool) {
        let suffix_len = if has_block { " {".len() } else { 0 };
        // A `@media` prelude is a comma-separated media-query list (Media Queries 4
        // §"media query list"); prettier breaks it at the commas (one query per
        // line) when it exceeds print width, so we do too.
        self.print_media_query_with_wrapping(content, suffix_len, MediaWrap::CommaList);
    }

    /// Format @supports/@container condition with line-width wrapping at `and`/`or` boundaries
    ///
    /// The `and`/`or` keyword stays on line 1, with the condition going to line 2.
    /// Example (wraps at 101 chars):
    /// ```css
    /// @supports (display: grid) and (transform: rotate(45deg)) and (filter: blur(5px)) and
    ///     (flex: 1aaa) {
    /// ```
    fn print_condition_query(
        &mut self,
        kind: ConditionKind<'_>,
        condition: &internal::ConditionQuery<'_>,
        has_block: bool,
        prelude_span: Option<tsv_lang::Span>,
    ) {
        // Print optional name prefix (for @container)
        let name_end_pos = if let Some(n) = kind.name() {
            self.write(n);
            self.write(" ");
            // Find where the name ends in source (name length from prelude start)
            prelude_span.map(|s| s.start + n.len() as u32)
        } else {
            prelude_span.map(|s| s.start)
        };

        // Normalize numbers in each part's content (`.5px` → `0.5px`), matching
        // the declaration-value path. Comments/strings within a part are
        // preserved; inter-part comments come from source spans, unaffected.
        let normalized_parts: Vec<NormalizedConditionPart<'_>> = condition
            .parts
            .iter()
            .map(|p| NormalizedConditionPart {
                connector: p.connector,
                content: if kind.normalizes() {
                    Cow::Owned(value_normalization::normalize_value_text(p.content))
                } else {
                    Cow::Borrowed(p.content)
                },
                span: p.span,
            })
            .collect();
        let parts = &normalized_parts;

        if parts.len() <= 1 {
            // Single condition - emit leading comments, content, and trailing comments
            if let Some(first_part) = parts.first() {
                self.write_leading_condition_comments(name_end_pos, first_part.span.start);
                self.write(&first_part.content);
                // Print trailing comments after the single part
                if let Some(span) = prelude_span {
                    self.write_trailing_condition_comments(first_part.span.end, span.end);
                }
            }
            return;
        }

        // Build doc to check if it fits on one line
        let prelude_doc = self.build_condition_doc(parts);
        let suffix_len = if has_block { " {".len() } else { 0 };

        let current_col = self.current_column();
        let available = PRINT_WIDTH.saturating_sub(current_col + suffix_len);
        let fits = doc::arena_fits::<dyn doc::TextResolver>(
            self.arena,
            prelude_doc,
            available,
            Mode::Flat,
            None,
        );

        if fits {
            // Print inline with comments between parts
            for (i, part) in parts.iter().enumerate() {
                if i > 0 {
                    self.write_condition_part_with_comments(parts[i - 1].span.end, part);
                } else {
                    self.write_leading_condition_comments(name_end_pos, part.span.start);
                    self.write_connector(part.connector);
                    self.write(&part.content);
                }
            }
            // Print trailing comments after last part
            if let (Some(last_part), Some(span)) = (parts.last(), prelude_span) {
                self.write_trailing_condition_comments(last_part.span.end, span.end);
            }
        } else {
            // Find split point: which part should start line 2
            let split_idx = self.find_condition_split_index(parts, current_col, suffix_len);

            // Print first line: parts[0..split_idx]
            for (i, part) in parts[..split_idx].iter().enumerate() {
                if i > 0 {
                    self.write_condition_part_with_comments(parts[i - 1].span.end, part);
                } else {
                    self.write_leading_condition_comments(name_end_pos, part.span.start);
                    self.write_connector(part.connector);
                    self.write(&part.content);
                }
            }

            // Print trailing connector and continuation line
            if split_idx < parts.len() {
                let prev_end = parts[split_idx - 1].span.end;
                let split_part = &parts[split_idx];
                let (before_conn, after_conn) = self.extract_comments_split_by_connector(
                    prev_end,
                    split_part.span.start,
                    split_part.connector,
                );

                if !before_conn.is_empty() {
                    self.write(" ");
                    self.write(&before_conn);
                }

                if let Some(conn) = split_part.connector {
                    self.write(" ");
                    self.write(connector_str(conn));
                }

                // Print continuation line
                self.write("\n");
                self.write_indent_extra(1);

                // Comments after connector go on the new line
                if !after_conn.is_empty() {
                    self.write(&after_conn);
                    self.write(" ");
                }

                self.write(&split_part.content);

                // Print remaining parts
                for (i, part) in parts[split_idx + 1..].iter().enumerate() {
                    self.write_condition_part_with_comments(parts[split_idx + i].span.end, part);
                }

                // Print trailing comments after last part
                if let (Some(last_part), Some(span)) = (parts.last(), prelude_span) {
                    self.write_trailing_condition_comments(last_part.span.end, span.end);
                }
            }
        }
    }

    /// Write comments that appear before the first condition part
    fn write_leading_condition_comments(&mut self, start_pos: Option<u32>, part_start: u32) {
        if let Some(start) = start_pos {
            let comments: Vec<_> = comments_in_range(self.comments, start, part_start).collect();
            if !comments.is_empty() {
                for (i, comment) in comments.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.write("/*");
                    self.write(comment.content(self.source));
                    self.write("*/");
                }
                self.write(" ");
            }
        }
    }

    /// Write comments that appear after the last condition part
    fn write_trailing_condition_comments(&mut self, last_part_end: u32, prelude_end: u32) {
        let comments: Vec<_> =
            comments_in_range(self.comments, last_part_end, prelude_end).collect();
        if !comments.is_empty() {
            for comment in comments.iter() {
                self.write(" /*");
                self.write(comment.content(self.source));
                self.write("*/");
            }
        }
    }

    /// Write a condition part with its preceding comments and connector
    fn write_condition_part_with_comments(
        &mut self,
        prev_end: u32,
        part: &NormalizedConditionPart<'_>,
    ) {
        let (before_conn, after_conn) =
            self.extract_comments_split_by_connector(prev_end, part.span.start, part.connector);

        if !before_conn.is_empty() {
            self.write(" ");
            self.write(&before_conn);
        }
        self.write(" ");
        self.write_connector(part.connector);
        if !after_conn.is_empty() {
            self.write(&after_conn);
            self.write(" ");
        }
        self.write(&part.content);
    }

    /// Extract comments from source range, split around connector keyword
    ///
    /// Returns (comments_before_connector, comments_after_connector)
    /// For `/* a */ and /* b */` returns (`/* a */`, `/* b */`)
    fn extract_comments_split_by_connector(
        &self,
        start: u32,
        end: u32,
        connector: Option<internal::ConditionConnector>,
    ) -> (String, String) {
        let comments: Vec<_> = comments_in_range(self.comments, start, end).collect();

        if comments.is_empty() {
            return (String::new(), String::new());
        }

        // Find the connector keyword position in the source range
        let connector_keyword = match connector {
            Some(internal::ConditionConnector::And) => "and",
            Some(internal::ConditionConnector::Or) => "or",
            None => {
                // No connector - all comments go to "before"
                let mut result = String::new();
                for (i, comment) in comments.iter().enumerate() {
                    if i > 0 {
                        result.push(' ');
                    }
                    result.push_str("/*");
                    result.push_str(comment.content(self.source));
                    result.push_str("*/");
                }
                return (result, String::new());
            }
        };

        // Find connector position in source (case-insensitive)
        let range_text = &self.source[start as usize..end as usize];
        let range_lower = range_text.to_lowercase();
        let connector_pos = range_lower
            .find(&format!(" {connector_keyword} "))
            .or_else(|| range_lower.find(connector_keyword));

        let connector_abs_pos = match connector_pos {
            Some(pos) => start + pos as u32,
            None => {
                // Connector not found - all comments go to "before"
                let mut result = String::new();
                for (i, comment) in comments.iter().enumerate() {
                    if i > 0 {
                        result.push(' ');
                    }
                    result.push_str("/*");
                    result.push_str(comment.content(self.source));
                    result.push_str("*/");
                }
                return (result, String::new());
            }
        };

        // Split comments based on whether they're before or after the connector
        let mut before = String::new();
        let mut after = String::new();

        for comment in comments {
            let formatted = format!("/*{}*/", comment.content(self.source));
            if comment.span.end <= connector_abs_pos {
                if !before.is_empty() {
                    before.push(' ');
                }
                before.push_str(&formatted);
            } else {
                if !after.is_empty() {
                    after.push(' ');
                }
                after.push_str(&formatted);
            }
        }

        (before, after)
    }

    /// Write a condition connector (if present) with trailing space
    fn write_connector(&mut self, connector: Option<internal::ConditionConnector>) {
        if let Some(conn) = connector {
            self.write(connector_str(conn));
            self.write(" ");
        }
    }

    /// Find the split index for condition query wrapping
    ///
    /// Returns the index of the first part that should go on line 2.
    fn find_condition_split_index(
        &self,
        parts: &[NormalizedConditionPart<'_>],
        current_col: usize,
        suffix_len: usize,
    ) -> usize {
        let mut line_width = current_col;

        for (i, part) in parts.iter().enumerate() {
            // Width of connector before this part (if any)
            let space_before = if i > 0 { 1 } else { 0 };
            let conn_width = if let Some(conn) = part.connector {
                space_before + connector_str(conn).len() + 1 // " and " or "and "
            } else {
                space_before // Just space between parts
            };

            let part_width = part.content.len();

            // Check if adding this part + suffix exceeds width
            let projected = line_width + conn_width + part_width + suffix_len;

            if projected > PRINT_WIDTH && i > 0 {
                // Split before this part
                return i;
            }

            line_width += conn_width + part_width;
        }

        parts.len()
    }

    /// Build a doc representation of condition query for width checking
    fn build_condition_doc(&self, parts: &[NormalizedConditionPart<'_>]) -> DocId {
        let d = self.d();
        let mut docs = DocBuf::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                docs.push(d.text(" "));
            }
            if let Some(conn) = part.connector {
                docs.push(d.text(connector_str(conn)));
                docs.push(d.text(" "));
            }
            docs.push(d.text_owned(part.content.to_string()));
        }

        d.concat(&docs)
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

    /// Format an @import media query with line-width wrapping.
    ///
    /// Prettier value-parses `@import` preludes (`isModuleRuleName`) and emits the
    /// media condition as `group(indent(fill(...)))`. A single query wraps only at
    /// the last fitting `and`/`or` (`print_media_query_with_wrapping`); a
    /// comma-separated query *list* packs greedily — see
    /// `print_import_media_query_fill`. The trailing `;` is the only suffix (1).
    fn print_import_media_query(&mut self, content: &str) {
        let normalized = value_normalization::normalize_value_text(content);
        // Fits inline (counting the trailing `;`) — emit verbatim.
        let total_width = self.current_column() + normalized.len() + 1;
        if total_width <= PRINT_WIDTH {
            self.write(&normalized);
            return;
        }
        // Comment-bearing conditions keep the comment-aware `and`/`or` wrapping — the
        // fill splits on whitespace and would shatter `/* … */` comments. (Comment-only
        // comma lists never reach here; the caller routes them to the value path.)
        if normalized.contains("/*") {
            self.print_media_query_with_wrapping(&normalized, 1, MediaWrap::AndOr);
            return;
        }
        let queries: Vec<&str> = value_normalization::split_args_by_comma(&normalized)
            .into_iter()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .collect();
        if queries.len() <= 1 {
            // Single query: wrap at the last fitting `and`/`or`.
            self.print_media_query_with_wrapping(&normalized, 1, MediaWrap::AndOr);
            return;
        }
        self.print_import_media_query_fill(&queries);
    }

    /// Greedy two-level fill for a comma-separated `@import` media-query list.
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

    /// Shared helper for media query wrapping.
    ///
    /// Used by both @media prelude and @import media conditions. `suffix_len`
    /// accounts for trailing content (` {` for @media, `;` for @import). For
    /// `MediaWrap::CommaList` (the `@media` query-list case) an overflowing prelude
    /// breaks each top-level comma-separated query onto its own line; otherwise
    /// (and for a single query) we wrap at the last fitting `and`/`or`.
    fn print_media_query_with_wrapping(
        &mut self,
        content: &str,
        suffix_len: usize,
        wrap: MediaWrap,
    ) {
        // Normalize numbers and string quotes in the raw prelude (`.5px` → `0.5px`,
        // `"x"` → `'x'`), matching the declaration-value path. Comments preserved.
        let content = value_normalization::normalize_value_text(content);
        let content = content.as_str();

        let current_col = self.current_column();
        let total_width = current_col + content.len() + suffix_len;

        if total_width <= PRINT_WIDTH {
            self.write(content);
            return;
        }

        // Comma-separated media-query list: break at every top-level comma, one
        // query per line (prettier's `group(indent(join(line, …)))`). A single
        // query (no top-level comma) falls through to `and`/`or` wrapping below.
        if matches!(wrap, MediaWrap::CommaList) {
            let queries: Vec<&str> = value_normalization::split_args_by_comma(content)
                .into_iter()
                .map(str::trim)
                .filter(|q| !q.is_empty())
                .collect();
            if queries.len() > 1 {
                for (i, query) in queries.iter().enumerate() {
                    if i > 0 {
                        self.write_indent_extra(1);
                    }
                    self.write(query);
                    if i + 1 < queries.len() {
                        self.write(",\n");
                    }
                }
                return;
            }
        }

        // Find the last `and`/`or` break point that keeps first line under print_width
        let mut best_break = None;

        for (idx, _) in content.match_indices(" and ") {
            let break_pos = idx + " and".len();
            if current_col + break_pos <= PRINT_WIDTH {
                best_break = Some(break_pos);
            }
        }

        for (idx, _) in content.match_indices(" or ") {
            let break_pos = idx + " or".len();
            if current_col + break_pos <= PRINT_WIDTH && best_break.is_none_or(|b| break_pos > b) {
                best_break = Some(break_pos);
            }
        }

        if let Some(break_pos) = best_break {
            self.write(&content[..break_pos]);
            self.write("\n");
            self.write_indent_extra(1);
            self.write(content[break_pos..].trim_start());
        } else {
            self.write(content);
        }
    }
}
