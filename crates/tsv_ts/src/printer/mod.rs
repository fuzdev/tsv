// TypeScript printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct and program printing orchestration
// - **statements.rs**: Statement printing (declarations, control flow, etc.)
// - **expressions.rs**: Expression printing (literals, identifiers, binary ops, etc.)
// - **types.rs**: Type annotation printing (TypeScript-specific type syntax)
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change TypeScript semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod arrays;
mod calls;
mod expressions;
mod objects;
mod operators;
mod statements;
mod types;

use crate::ast::internal;
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::DefaultStringInterner;
use tsv_lang::{
    OutputBuffer, PrintConfig, SymbolResolver, comments_in_range, has_comments_in_range,
    has_line_comments_in_range, printing,
};

/// Check if an expression is a pure property chain (member expressions without calls)
///
/// Pure property chains like `obj.a.b.c` should use fluid assignment wrapping
/// (break after `=` if doesn't fit). Expressions containing calls, objects,
/// arrays, or ternaries handle their own wrapping internally.
pub(crate) fn is_pure_property_chain(expr: &internal::Expression) -> bool {
    match expr {
        // A member expression is a property chain if its object is also a pure chain
        internal::Expression::MemberExpression(member) => is_pure_property_chain(&member.object),
        // Base case: identifiers are valid chain roots
        internal::Expression::Identifier(_) => true,
        // Everything else (calls, objects, arrays, ternaries, etc.) is NOT a pure chain
        _ => false,
    }
}

/// Check if an expression is a multiline string literal (contains line continuations)
///
/// Strings with `\<newline>` need fluid layout because:
/// 1. They span multiple lines in source
/// 2. Prettier always wraps the declaration for these
pub(crate) fn is_multiline_string_literal(expr: &internal::Expression, source: &str) -> bool {
    if let internal::Expression::Literal(lit) = expr
        && let internal::LiteralValue::String { .. } = &lit.value
    {
        let raw = lit.span.extract(source);
        // Check for line continuation: backslash followed by newline
        return raw.contains("\\\n") || raw.contains("\\\r");
    }
    false
}

/// Check if an expression contains multiline string literals at any depth
/// Check if an object pattern should expand (print across multiple lines).
///
/// Prettier expands object patterns when any property has a nested pattern
/// (ObjectPattern or ArrayPattern) as its value. This is different from the
/// multiline content rule used for object expressions.
pub(crate) fn object_pattern_should_expand(obj: &internal::ObjectPattern) -> bool {
    obj.properties.iter().any(|prop| match prop {
        internal::ObjectPatternProperty::Property(p) => {
            // Check if the value is a nested pattern
            matches!(
                p.value,
                internal::Expression::ObjectPattern(_) | internal::Expression::ArrayPattern(_)
            ) ||
            // Also check AssignmentPattern with nested pattern left side
            matches!(&p.value, internal::Expression::AssignmentPattern(ap)
                if matches!(ap.left.as_ref(),
                    internal::Expression::ObjectPattern(_) | internal::Expression::ArrayPattern(_)))
        }
        internal::ObjectPatternProperty::RestElement(_) => false,
    })
}

///
/// Recursively traverses nested structures (arrays, objects, calls) to find
/// multiline strings. Prettier expands ALL containing structures when a multiline
/// string is found anywhere in the tree.
pub(crate) fn has_multiline_content(expr: &internal::Expression, source: &str) -> bool {
    match expr {
        internal::Expression::Literal(_) => is_multiline_string_literal(expr, source),
        internal::Expression::ArrayExpression(arr) => arr
            .elements
            .iter()
            .flatten()
            .any(|elem| has_multiline_content(elem, source)),
        internal::Expression::ObjectExpression(obj) => {
            obj.properties.iter().any(|prop| match prop {
                internal::ObjectProperty::Property(p) => has_multiline_content(&p.value, source),
                internal::ObjectProperty::SpreadElement(s) => {
                    has_multiline_content(&s.argument, source)
                }
            })
        }
        internal::Expression::CallExpression(call) => call
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, source)),
        internal::Expression::NewExpression(new_expr) => new_expr
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, source)),
        internal::Expression::UnaryExpression(unary) => {
            has_multiline_content(&unary.argument, source)
        }
        internal::Expression::UpdateExpression(update) => {
            has_multiline_content(&update.argument, source)
        }
        internal::Expression::BinaryExpression(binary) => {
            has_multiline_content(&binary.left, source)
                || has_multiline_content(&binary.right, source)
        }
        internal::Expression::ConditionalExpression(cond) => {
            has_multiline_content(&cond.test, source)
                || has_multiline_content(&cond.consequent, source)
                || has_multiline_content(&cond.alternate, source)
        }
        internal::Expression::MemberExpression(member) => {
            has_multiline_content(&member.object, source)
        }
        internal::Expression::ArrowFunctionExpression(arrow) => match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => has_multiline_content(expr, source),
            internal::ArrowFunctionBody::BlockStatement(_) => false,
        },
        internal::Expression::SpreadElement(spread) => {
            has_multiline_content(&spread.argument, source)
        }
        internal::Expression::Identifier(_) => false,
        internal::Expression::TemplateLiteral(template) => {
            // Template literals with actual newlines in their content are multiline
            template.quasis.iter().any(|q| q.raw.contains('\n'))
                || template
                    .expressions
                    .iter()
                    .any(|e| has_multiline_content(e, source))
        }
        internal::Expression::TaggedTemplateExpression(tagged) => {
            has_multiline_content(&tagged.tag, source)
                || tagged.quasi.quasis.iter().any(|q| q.raw.contains('\n'))
                || tagged
                    .quasi
                    .expressions
                    .iter()
                    .any(|e| has_multiline_content(e, source))
        }
        // Function expressions don't contribute to multiline content detection
        // They have their own block formatting
        internal::Expression::FunctionExpression(_) => false,
        internal::Expression::AwaitExpression(await_expr) => {
            has_multiline_content(&await_expr.argument, source)
        }
        internal::Expression::SequenceExpression(seq) => seq
            .expressions
            .iter()
            .any(|e| has_multiline_content(e, source)),
        // Regex literals don't have multiline content
        internal::Expression::RegexLiteral(_) => false,
        // Super is just a keyword, no multiline content
        internal::Expression::Super(_) => false,
        // Assignment expression: check both sides
        internal::Expression::AssignmentExpression(assign) => {
            has_multiline_content(&assign.left, source)
                || has_multiline_content(&assign.right, source)
        }
        // Patterns: check their contents
        internal::Expression::ObjectPattern(obj) => obj.properties.iter().any(|prop| match prop {
            internal::ObjectPatternProperty::Property(p) => has_multiline_content(&p.value, source),
            internal::ObjectPatternProperty::RestElement(r) => {
                has_multiline_content(&r.argument, source)
            }
        }),
        internal::Expression::ArrayPattern(arr) => arr
            .elements
            .iter()
            .flatten()
            .any(|elem| has_multiline_content(elem, source)),
        internal::Expression::AssignmentPattern(pattern) => {
            has_multiline_content(&pattern.left, source)
                || has_multiline_content(&pattern.right, source)
        }
        internal::Expression::RestElement(rest) => has_multiline_content(&rest.argument, source),
    }
}

/// Printer state for building output
pub struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    #[allow(dead_code)]
    pub(crate) indent_level: usize,
    /// Print configuration
    #[allow(dead_code)]
    config: PrintConfig,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
    /// Original source code (for extracting raw values, preserving escape sequences, etc.)
    pub(crate) source: &'a str,
    /// Comments from the program (for printing leading/trailing comments)
    pub(crate) comments: &'a Vec<internal::Comment>,
    /// Extra indent depth for declaration contexts (0 normally, 1+ in multi-declarator)
    /// When > 0, multiline objects/arrays get extra indentation
    pub(crate) declaration_indent_depth: usize,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given interner, source, comments, and config
    pub fn with_config(
        interner: Rc<RefCell<DefaultStringInterner>>,
        source: &'a str,
        comments: &'a Vec<internal::Comment>,
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            interner,
            source,
            comments,
            declaration_indent_depth: 0,
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write indentation based on current indent level
    #[allow(dead_code)]
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, self.config.indent);
    }

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Set the indent level (for formatting expressions in nested contexts)
    pub fn set_indent_level(&mut self, level: usize) {
        self.indent_level = level;
    }

    /// Get the current column position (for doc-builder width calculations)
    pub(crate) fn current_column(&self) -> usize {
        self.buffer.current_column(self.config.tab_width)
    }

    /// Build a Doc for an expression (public wrapper for doc-based formatting)
    ///
    /// This returns a Doc tree that can be used for line wrapping decisions
    /// when embedding TS expressions in larger documents (e.g., Svelte attributes).
    pub fn build_expression_doc_public(&self, expr: &internal::Expression) -> tsv_lang::doc::Doc {
        self.build_expression_doc(expr)
    }

    /// Check if a pattern expression should expand (print across multiple lines)
    #[allow(clippy::only_used_in_recursion)]
    pub(crate) fn pattern_should_expand(&self, expr: &internal::Expression) -> bool {
        match expr {
            internal::Expression::ObjectPattern(obj) => object_pattern_should_expand(obj),
            internal::Expression::ArrayPattern(_) => false, // Array patterns don't expand based on nesting
            internal::Expression::AssignmentPattern(ap) => self.pattern_should_expand(&ap.left),
            _ => false,
        }
    }

    /// Print a TypeScript comment
    pub(crate) fn print_comment(&mut self, comment: &internal::Comment) {
        if comment.is_block {
            // Block comment: /* content */
            self.write("/*");

            // Check if multi-line - if so, strip and re-apply indentation
            if comment.content.contains('\n') {
                let stripped = printing::strip_comment_indentation(
                    self.source,
                    &comment.content,
                    comment.span.start,
                );
                let lines: Vec<&str> = stripped.split('\n').collect();
                for (i, line) in lines.iter().enumerate() {
                    if i > 0 {
                        // Add newline and indentation for subsequent lines
                        self.write("\n");
                        self.write_indent();
                    }
                    self.write(line);
                }
            } else {
                // Single-line block comment
                self.write(&comment.content);
            }

            self.write("*/");
        } else {
            // Line comment: // content (no closing delimiter)
            self.write("//");
            self.write(&comment.content);
        }
    }

    /// Print a TypeScript program
    pub fn print_program(&mut self, program: &internal::Program) {
        let mut prev_end = 0u32; // Start of file

        for (i, statement) in program.body.iter().enumerate() {
            // Always add newline between statements (separator)
            if i > 0 {
                self.write("\n");

                // Check for blank lines between statements (preserve from source)
                // Only check if there are no comments between statements
                // (comments handle their own blank line preservation)
                let has_comments = self
                    .comments
                    .iter()
                    .any(|c| c.span.start >= prev_end && c.span.end <= statement.span().start);

                if !has_comments
                    && printing::has_blank_line_between(
                        self.source,
                        prev_end,
                        statement.span().start,
                    )
                {
                    self.write("\n");
                }
            }

            // Print leading comments before this statement
            // (blank line preservation handled inside print_leading_comments)
            // For the first statement, include same-line comments (no previous statement to be trailing from)
            let is_first = i == 0;
            self.print_leading_comments(prev_end, statement.span().start, is_first);

            self.print_statement(statement);

            prev_end = statement.span().end;
        }

        // Add trailing newline (matches prettier)
        self.write("\n");
    }

    /// Print leading comments (comments between prev_end and curr_start)
    /// Returns true if any comments were printed
    ///
    /// - `prev_end`: Position after the previous statement (or 0 for first statement)
    /// - `curr_start`: Position of the current statement
    /// - `is_first`: True if this is the first statement (prev_end is start of file)
    ///
    /// Uses binary search to find starting point: O(log n + k)
    fn print_leading_comments(&mut self, prev_end: u32, curr_start: u32, is_first: bool) -> bool {
        let mut last_comment_end = prev_end;
        let mut printed_any = false;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            // Skip comments that are on the same line as prev_end
            // (those are trailing inline comments, already printed)
            // EXCEPT for the first statement: same-line comments at start of file are leading comments
            if !is_first && printing::is_same_line(self.source, prev_end, comment.span.start) {
                continue;
            }

            // Check if comment is on the same line as curr_start
            // (those are same-line leading comments, print inline without newline)
            if printing::is_same_line(self.source, comment.span.end, curr_start) {
                self.write_indent();
                self.print_comment(comment);
                self.write(" ");
                printed_any = true;
                last_comment_end = comment.span.end;
                continue;
            }

            // Check if we need a blank line before this comment
            if comment.span.start > last_comment_end
                && printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                )
            {
                self.write("\n");
            }

            self.write_indent();
            self.print_comment(comment);
            self.write("\n");

            // Update last_comment_end for next iteration
            last_comment_end = comment.span.end;
            printed_any = true;
        }

        // Check if there's a blank line after the last comment and before curr_start
        if printed_any
            && last_comment_end < curr_start
            && printing::has_blank_line_between(self.source, last_comment_end, curr_start)
        {
            self.write("\n");
        }

        printed_any
    }

    /// Print inline comments in statement (handles comments before and after semicolon)
    ///
    /// Matches prettier's behavior: comments before semicolon are moved to after it.
    /// Example: `const x = 1 /* comment */;` → `const x = 1; /* comment */`
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn print_inline_comments_in_statement(&mut self, expr_end: u32, stmt_end: u32) {
        // Use binary search to skip comments before expr_end
        let first_idx = tsv_lang::find_first_comment_from(self.comments, expr_end);

        let mut has_comments = false;
        for comment in &self.comments[first_idx..] {
            // Print comments that are either:
            // 1. Between expression end and statement end (before semicolon), OR
            // 2. After statement end but on the same line (after semicolon)
            let in_range = comment.span.start >= expr_end && comment.span.end <= stmt_end;
            let same_line_after = comment.span.start >= stmt_end
                && printing::is_same_line(self.source, stmt_end, comment.span.start);

            if in_range || same_line_after {
                if !has_comments {
                    self.write(" ");
                    has_comments = true;
                }
                self.print_comment(comment);
            } else if comment.span.start > stmt_end {
                // Stop once we're past stmt_end and no longer on same line
                break;
            }
        }
    }

    /// Find the position of `=` character in the source between two positions
    pub(crate) fn find_equals_position(&self, start: u32, end: u32) -> u32 {
        let start = start as usize;
        let end = end as usize;
        let slice = &self.source[start..end];

        if let Some(offset) = slice.find('=') {
            (start + offset) as u32
        } else {
            // Fallback: return midpoint if `=` not found
            usize::midpoint(start, end) as u32
        }
    }

    /// Check if there are comments between two positions (read-only check)
    ///
    /// Uses binary search: O(log n)
    pub(crate) fn has_comments_between(&self, start: u32, end: u32) -> bool {
        has_comments_in_range(self.comments, start, end)
    }

    /// Check if there are line comments (// style) between two positions
    ///
    /// Uses binary search: O(log n + k) where k is comments in range
    pub(crate) fn has_line_comments_between(&self, start: u32, end: u32) -> bool {
        has_line_comments_in_range(self.comments, start, end)
    }

    /// Print inline comments between two positions (same-line comments only)
    /// Returns true if any comments were printed
    /// Note: Adds space before each comment, but NOT after (caller handles trailing space)
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn print_inline_comments_between(&mut self, start: u32, end: u32) -> bool {
        let mut printed_any = false;
        for comment in comments_in_range(self.comments, start, end) {
            self.write(" ");
            self.print_comment(comment);
            printed_any = true;
        }
        printed_any
    }

    /// Print leading comments for a variable declarator (no leading space)
    /// Returns true if any comments were printed
    /// Used for comments between declarators: `const a = 1, /* comment */ b = 2`
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn print_leading_comments_for_declarator(&mut self, start: u32, end: u32) -> bool {
        let mut printed_any = false;
        for comment in comments_in_range(self.comments, start, end) {
            self.print_comment(comment);
            printed_any = true;
        }
        printed_any
    }

    /// Print leading comments before an object property
    ///
    /// Different from `print_leading_comments` because it handles same-line leading comments
    /// (like `{/* comment */ a: 1}`), but still skips trailing comments from the previous property.
    ///
    /// - `prev_end`: Position after the previous property's value (or opening brace for first property)
    /// - `curr_start`: Position of the current property's key
    /// - `is_first_prop`: True if this is the first property (prev_end is opening brace position)
    ///
    /// Returns true if a same-line leading comment was printed (caller should skip its indent).
    /// For comments on their own line, prints with proper indentation and newline.
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn print_object_leading_comments(
        &mut self,
        prev_end: u32,
        curr_start: u32,
        is_first_prop: bool,
    ) -> bool {
        let mut last_comment_end = prev_end;
        let mut printed_same_line = false;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            // Skip comments on the same line as prev_end - those are trailing comments
            // for the previous property (already printed by the trailing comment logic).
            // EXCEPT for the first property: same-line comments after `{` are leading comments.
            if !is_first_prop && printing::is_same_line(self.source, prev_end, comment.span.start) {
                continue;
            }

            // Check if comment is on the same line as curr_start
            // (inline leading comment like `{/* comment */ a: 1}`)
            if printing::is_same_line(self.source, comment.span.end, curr_start) {
                // Same-line comment: always print indent (we're at start of a new line),
                // then comment and space
                self.write_indent();
                self.print_comment(comment);
                self.write(" ");
                last_comment_end = comment.span.end;
                printed_same_line = true;
                continue;
            }

            // Comment on its own line: check for blank lines
            if comment.span.start > last_comment_end
                && printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                )
            {
                self.write("\n");
            }

            self.write_indent();
            self.print_comment(comment);
            self.write("\n");

            last_comment_end = comment.span.end;
        }

        // Check for blank line after the last comment before the property
        if last_comment_end > prev_end
            && last_comment_end < curr_start
            && printing::has_blank_line_between(self.source, last_comment_end, curr_start)
        {
            self.write("\n");
        }

        printed_same_line
    }

    /// Print leading comments before a statement in a block
    ///
    /// Similar to print_leading_comments but handles the first statement specially:
    /// for the first statement, same-line comments after `{` are leading comments,
    /// not trailing comments from a previous statement.
    ///
    /// - `prev_end`: Position after the previous statement (or opening brace for first statement)
    /// - `curr_start`: Position of the current statement
    /// - `is_first`: True if this is the first statement (prev_end is after opening brace)
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn print_block_leading_comments(
        &mut self,
        prev_end: u32,
        curr_start: u32,
        is_first: bool,
    ) {
        let mut last_comment_end = prev_end;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            // Skip comments on the same line as prev_end - those are trailing comments
            // for the previous statement (already printed by the trailing comment logic).
            // EXCEPT for the first statement: same-line comments after `{` are leading comments.
            if !is_first && printing::is_same_line(self.source, prev_end, comment.span.start) {
                continue;
            }

            // Check for blank lines before this comment
            if comment.span.start > last_comment_end
                && printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                )
            {
                self.write("\n");
            }

            self.write_indent();
            self.print_comment(comment);
            self.write("\n");

            last_comment_end = comment.span.end;
        }

        // Check for blank line after the last comment before the statement
        if last_comment_end > prev_end
            && last_comment_end < curr_start
            && printing::has_blank_line_between(self.source, last_comment_end, curr_start)
        {
            self.write("\n");
        }
    }
}

// Implement SymbolResolver trait for shared symbol resolution utilities
impl<'a> SymbolResolver for Printer<'a> {
    fn interner(&self) -> &Rc<RefCell<DefaultStringInterner>> {
        &self.interner
    }
}
