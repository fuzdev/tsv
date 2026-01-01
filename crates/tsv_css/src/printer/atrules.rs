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

use super::Printer;
use crate::ast::internal;
use tsv_lang::{comments_in_range, doc, printing};

/// Convert a supports connector to its string representation
fn connector_str(conn: internal::SupportsConnector) -> &'static str {
    match conn {
        internal::SupportsConnector::And => "and",
        internal::SupportsConnector::Or => "or",
    }
}

impl<'a> Printer<'a> {
    /// Format a CSS at-rule (@media, @keyframes, @supports, etc.)
    pub(super) fn print_css_atrule(&mut self, atrule: &internal::CssAtrule) {
        self.write("@");
        self.write(&atrule.name);

        // Print prelude based on type
        match &atrule.prelude {
            internal::PreludeValue::Values { values, .. } if !values.is_empty() => {
                self.write(" ");
                // Special handling for @import with media query (last value may need wrapping)
                let is_import = atrule.name == "import";
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    // Check if this is the media query part of @import that needs wrapping
                    if is_import
                        && i == values.len() - 1
                        && let internal::CssValue::Identifier { name, .. } = value
                    {
                        // Check if it's a media query (contains "and" or "or")
                        if name.contains(" and ") || name.contains(" or ") {
                            self.print_import_media_query(name);
                            continue;
                        }
                    }
                    // Use doc-based formatting to normalize quotes and spacing
                    self.print_nested_value(value);
                }
            }
            internal::PreludeValue::Raw { content, .. } if !content.is_empty() => {
                self.write(" ");
                let normalized = self.normalize_comment_spacing(content);
                self.write(&normalized);
            }
            internal::PreludeValue::Supports { condition, span } => {
                self.write(" ");
                self.print_condition_query(None, condition, atrule.block.is_some(), Some(*span));
            }
            internal::PreludeValue::Container {
                name,
                condition,
                span,
            } => {
                self.write(" ");
                self.print_condition_query(
                    name.as_deref(),
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
            while i < block.children.len() {
                let child = &block.children[i];

                // For non-first children, add newline before rules/at-rules/comments
                if i > 0 {
                    match child {
                        internal::CssBlockChild::Declaration(_) => {
                            // Declarations already end with \n, no extra newline needed
                        }
                        internal::CssBlockChild::Rule(_)
                        | internal::CssBlockChild::Atrule(_)
                        | internal::CssBlockChild::Comment(_) => {
                            let prev_child = &block.children[i - 1];
                            let has_blank_line = self.has_blank_line_between_spans(
                                prev_child.span().end,
                                child.span().start,
                            );

                            // Declarations end with \n, but rules/at-rules end with }
                            // So only add separator newline if prev is not a declaration
                            let prev_is_declaration =
                                matches!(prev_child, internal::CssBlockChild::Declaration(_));

                            if !prev_is_declaration {
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
                        self.print_atrule_block_child(child);

                        // Check if next child is an inline comment
                        let inline_count =
                            self.try_print_inline_comments(&block.children, i, child.span().end);
                        i += inline_count;
                    }
                    internal::CssBlockChild::Comment(_) => {
                        // Standalone comment
                        self.write_indent();
                        self.print_atrule_block_child(child);
                    }
                }

                i += 1;
            }

            self.indent_level -= 1;

            // Only write newline if the last child wasn't a declaration
            // (declarations end with \n already)
            if !matches!(
                block.children.last(),
                Some(internal::CssBlockChild::Declaration(_))
            ) {
                self.write("\n");
            }
            self.write_indent();
            self.write("}");
        } else {
            self.write(";");
        }
    }

    /// Format an at-rule block child (rule, declaration, or nested at-rule)
    fn print_atrule_block_child(&mut self, child: &internal::CssBlockChild) {
        match child {
            internal::CssBlockChild::Rule(rule) => {
                // Format rule selector and opening brace
                self.print_selector_list(&rule.selector);

                // Check if first child is a comment after selector (before {)
                let mut start_index = 0;
                if let Some(internal::CssBlockChild::Comment(comment)) = rule.declarations.first() {
                    // Check if comment is on same line as selector AND before the opening brace
                    // If there's a '{' between selector and comment, the comment is inside the block, not after selector
                    if printing::is_same_line(
                        self.source,
                        rule.selector.span.end,
                        comment.span.start,
                    ) && !self
                        .has_opening_brace_between(rule.selector.span.end, comment.span.start)
                    {
                        // Print comment inline after selector
                        self.write(" /*");
                        self.write(&comment.content);
                        self.write("*/");
                        start_index = 1; // Skip this comment when processing declarations
                    }
                }

                self.write(" {\n");

                // Format declarations and comments with proper indentation
                self.indent_level += 1;
                let mut i = start_index;
                while i < rule.declarations.len() {
                    let block_child = &rule.declarations[i];
                    match block_child {
                        internal::CssBlockChild::Declaration(decl) => {
                            self.print_css_declaration(decl);

                            // Check for inline comments after the declaration
                            let inline_count = self.try_print_inline_comments_after_decl(
                                &rule.declarations,
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
                                && let Some(prev_child) = rule.declarations.get(i - 1)
                                && self.has_blank_line_between_spans(
                                    prev_child.span().end,
                                    comment.span.start,
                                )
                            {
                                // Source has blank line - add it
                                // Note: Previous element already ended with \n, so one more \n gives blank line
                                self.write("\n");
                            }
                            self.write_indent();
                            self.print_css_comment(comment);
                            self.write("\n");
                        }
                        internal::CssBlockChild::Rule(nested_rule) => {
                            // CSS Nesting Module - format nested rule inside at-rule block rule
                            if i > start_index && !Self::prev_is_comment(&rule.declarations, i) {
                                self.write("\n");
                            }
                            self.write_indent();
                            self.print_css_rule(nested_rule);

                            // Check for inline comment after nested rule's closing brace
                            let inline_count = self.try_print_inline_comments(
                                &rule.declarations,
                                i,
                                nested_rule.span.end,
                            );

                            self.write("\n");

                            // Add blank line after nested rule if next sibling is a declaration
                            // (Don't add for comments - comment handles its own spacing)
                            let next_idx = i + 1 + inline_count;
                            if let Some(internal::CssBlockChild::Declaration(_)) =
                                rule.declarations.get(next_idx)
                            {
                                self.write("\n");
                            }

                            i += inline_count;
                        }
                        internal::CssBlockChild::Atrule(nested_atrule) => {
                            // Nested at-rule inside rule
                            if i > start_index && !Self::prev_is_comment(&rule.declarations, i) {
                                self.write("\n");
                            }
                            self.write_indent();
                            self.print_css_atrule(nested_atrule);
                            self.write("\n");

                            // Add blank line after nested at-rule if next sibling is a declaration
                            // (Don't add for comments - comment handles its own spacing)
                            if let Some(next_child) = rule.declarations.get(i + 1)
                                && matches!(next_child, internal::CssBlockChild::Declaration(_))
                            {
                                self.write("\n");
                            }
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
        self.print_media_query_with_wrapping(content, suffix_len);
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
        name: Option<&str>,
        condition: &internal::SupportsCondition,
        has_block: bool,
        prelude_span: Option<tsv_lang::Span>,
    ) {
        // Print optional name prefix (for @container)
        let name_end_pos = if let Some(n) = name {
            self.write(n);
            self.write(" ");
            // Find where the name ends in source (name length from prelude start)
            prelude_span.map(|s| s.start + n.len() as u32)
        } else {
            prelude_span.map(|s| s.start)
        };

        let parts = &condition.parts;

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
        let available = self
            .config
            .print_width
            .saturating_sub(current_col + suffix_len);
        let fits = doc::fits(&prelude_doc, available, doc::Mode::Flat, &self.config);

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
                self.indent_level += 1;
                self.write_indent();
                self.indent_level -= 1;

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
                    self.write(&comment.content);
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
                self.write(&comment.content);
                self.write("*/");
            }
        }
    }

    /// Write a condition part with its preceding comments and connector
    fn write_condition_part_with_comments(&mut self, prev_end: u32, part: &internal::SupportsPart) {
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
        connector: Option<internal::SupportsConnector>,
    ) -> (String, String) {
        let comments: Vec<_> = comments_in_range(self.comments, start, end).collect();

        if comments.is_empty() {
            return (String::new(), String::new());
        }

        // Find the connector keyword position in the source range
        let connector_keyword = match connector {
            Some(internal::SupportsConnector::And) => "and",
            Some(internal::SupportsConnector::Or) => "or",
            None => {
                // No connector - all comments go to "before"
                let mut result = String::new();
                for (i, comment) in comments.iter().enumerate() {
                    if i > 0 {
                        result.push(' ');
                    }
                    result.push_str("/*");
                    result.push_str(&comment.content);
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
                    result.push_str(&comment.content);
                    result.push_str("*/");
                }
                return (result, String::new());
            }
        };

        // Split comments based on whether they're before or after the connector
        let mut before = String::new();
        let mut after = String::new();

        for comment in comments {
            let formatted = format!("/*{}*/", comment.content);
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
    fn write_connector(&mut self, connector: Option<internal::SupportsConnector>) {
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
        parts: &[internal::SupportsPart],
        current_col: usize,
        suffix_len: usize,
    ) -> usize {
        let mut line_width = current_col;
        let print_width = self.config.print_width;

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

            if projected > print_width && i > 0 {
                // Split before this part
                return i;
            }

            line_width += conn_width + part_width;
        }

        parts.len()
    }

    /// Build a doc representation of condition query for width checking
    fn build_condition_doc(&self, parts: &[internal::SupportsPart]) -> doc::Doc {
        let mut docs = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                docs.push(doc::text(" "));
            }
            if let Some(conn) = part.connector {
                docs.push(doc::text(connector_str(conn)));
                docs.push(doc::text(" "));
            }
            docs.push(doc::text_owned(part.content.clone()));
        }

        doc::concat(docs)
    }

    /// Format @import media query with line-width wrapping at `and`/`or` boundaries
    ///
    /// Similar to `print_media_prelude` but for the media query part of @import.
    /// Wraps at >100 chars (Prettier waits until >102).
    fn print_import_media_query(&mut self, content: &str) {
        // @import has no block, suffix is just the semicolon
        self.print_media_query_with_wrapping(content, 1);
    }

    /// Shared helper for media query wrapping at `and`/`or` boundaries
    ///
    /// Used by both @media prelude and @import media conditions.
    /// `suffix_len` accounts for trailing content (` {` for @media, `;` for @import).
    fn print_media_query_with_wrapping(&mut self, content: &str, suffix_len: usize) {
        let current_col = self.current_column();
        let total_width = current_col + content.len() + suffix_len;

        if total_width <= self.config.print_width {
            self.write(content);
            return;
        }

        // Find the last `and`/`or` break point that keeps first line under print_width
        let mut best_break = None;

        for (idx, _) in content.match_indices(" and ") {
            let break_pos = idx + " and".len();
            if current_col + break_pos <= self.config.print_width {
                best_break = Some(break_pos);
            }
        }

        for (idx, _) in content.match_indices(" or ") {
            let break_pos = idx + " or".len();
            if current_col + break_pos <= self.config.print_width
                && best_break.is_none_or(|b| break_pos > b)
            {
                best_break = Some(break_pos);
            }
        }

        if let Some(break_pos) = best_break {
            self.write(&content[..break_pos]);
            self.write("\n");
            self.indent_level += 1;
            self.write_indent();
            self.indent_level -= 1;
            self.write(content[break_pos..].trim_start());
        } else {
            self.write(content);
        }
    }
}
