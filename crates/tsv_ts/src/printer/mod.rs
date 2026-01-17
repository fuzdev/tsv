// TypeScript printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct and program printing orchestration
// - **analysis.rs**: Pure AST analysis functions (no Printer state needed)
// - **comments.rs**: Comment handling (printing, doc building, filtering)
// - **statements/**: Statement printing (declarations, control flow, modules, etc.)
// - **expressions/**: Expression printing (dispatchers, literals, functions, patterns, templates)
// - **types.rs**: Type annotation printing (TypeScript-specific type syntax)
// - **objects.rs**: Object expression printing
// - **arrays.rs**: Array expression printing
// - **operators.rs**: Unary and binary expression printing
// - **calls.rs**: Call, member, and conditional expression printing
// - **expression_stringifier.rs**: Generic expression-to-string conversion utility
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change TypeScript semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod analysis;
mod arrays;
mod assignment;
mod calls;
mod chain;
mod comments;
mod conditional;
mod expression_stringifier;
mod expressions;
mod needs_parens;
mod new_expression;
mod objects;
mod operators;
mod statements;
mod type_stringifier;
mod types;
mod utils;

pub(crate) use analysis::{
    PatternContext, build_entity_name_doc, conditional_needs_fluid_layout, has_multiline_content,
    is_brace_block_multiline, is_module_path_fluid_call, is_multiline_string_literal,
    is_plain_require_call, is_pure_property_chain, object_pattern_should_expand,
    template_literal_has_newlines,
};
pub(crate) use assignment::{
    is_poorly_breakable_chain, is_self_expanding_value, is_simple_self_expanding,
};
pub(crate) use comments::{CommentFilter, CommentSpacing};
pub(crate) use needs_parens::{ParenContext, needs_parens};
pub(crate) use types::{intersection_has_huggable_last_type, unwrap_parenthesized};

use crate::ast::internal;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use string_interner::DefaultStringInterner;
use tsv_lang::{
    CommentPosition, OutputBuffer, PrintConfig, SymbolResolver, classify_comment, comments_after,
    comments_in_range,
    doc::{self, Doc},
    has_comments_in_range, has_line_comments_in_range, printing,
};

/// Printer state for building output
pub struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Print configuration
    config: PrintConfig,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
    /// Original source code (for extracting raw values, preserving escape sequences, etc.)
    pub(crate) source: &'a str,
    /// Comments from the program (for printing leading/trailing comments)
    pub(crate) comments: &'a [internal::Comment],
    /// Extra indent depth for declaration contexts (0 normally, 1+ in multi-declarator)
    /// When > 0, multiline objects/arrays get extra indentation
    /// Uses Cell for interior mutability so doc builders (&self) can set this
    pub(crate) declaration_indent_depth: Cell<usize>,
    /// Whether we're currently inside an expression statement (for chain merging decisions)
    /// Uses Cell for interior mutability so doc builders (&self) can set this
    pub(crate) is_expression_statement: Cell<bool>,
    /// Whether we're in a top-level assignment context (ExpressionStatement or VariableDeclaration)
    /// Used for assignment chain detection - assignments at top level use regular grouped layout,
    /// only nested assignments (where parent is another assignment) use chain formatting
    /// Uses Cell for interior mutability so doc builders (&self) can set this
    pub(crate) in_top_level_assignment: Cell<bool>,
    /// Whether to force chain expressions to expand (break at every method call)
    /// Set when inside template expressions with original breaks, where the chain
    /// would exceed print width if kept flat.
    pub(crate) force_chain_expand: Cell<bool>,
    /// Whether we're inside a template literal interpolation (${ ... })
    /// Used to collapse blank lines - in template interpolations, blank lines are
    /// normalized to single line breaks rather than preserved.
    pub(crate) in_template_interpolation: Cell<bool>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given interner, source, comments, and config
    pub fn with_config(
        interner: Rc<RefCell<DefaultStringInterner>>,
        source: &'a str,
        comments: &'a [internal::Comment],
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            config,
            interner,
            source,
            comments,
            declaration_indent_depth: Cell::new(0),
            is_expression_statement: Cell::new(false),
            in_top_level_assignment: Cell::new(false),
            force_chain_expand: Cell::new(false),
            in_template_interpolation: Cell::new(false),
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write a Doc to the buffer, accounting for current column and indent level
    ///
    /// This handles the common pattern of:
    /// 1. Calculate current column with context offset
    /// 2. Print doc with indent-aware width calculations
    /// 3. Write the result to the buffer
    ///
    /// For width calculations, we account for outer context in two ways:
    /// - If `first_line_offset > 0`: expression is embedded inline (e.g., Svelte block), use it directly
    /// - If `first_line_offset == 0`: standalone block (e.g., `<script>`), use `base_indent_offset * tab_width`
    ///
    /// Use `write_doc` for expression statements where the semicolon
    /// (like semicolons) should be considered in width calculations.
    pub(crate) fn write_doc(&mut self, d: &Doc) {
        // Calculate offset to account for outer context in width calculations
        let context_offset = if self.config.first_line_offset > 0 {
            // Inline embedded expression: first_line_offset includes visual width of outer indent
            if self.current_column() == 0 {
                self.config.first_line_offset
            } else {
                0
            }
        } else {
            // Standalone block: account for outer indent not visible in our buffer
            self.config.base_indent_offset * self.config.tab_width
        };
        let current_col = self.current_column() + context_offset;
        let output = {
            let interner = self.interner.borrow();
            doc::print_doc_with_indent_resolved(
                d,
                &self.config,
                current_col,
                self.indent_level,
                &*interner,
            )
        };
        self.write(&output);
    }

    /// Write indentation based on current indent level
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

    /// Render a Doc to a flat string with effectively infinite width.
    ///
    /// Used when we need to prevent a doc from breaking internally,
    /// such as expressions inside template literal interpolations.
    pub(crate) fn render_doc_flat(&self, d: &Doc) -> String {
        // Use MAX/2 instead of MAX to avoid potential overflow in width calculations
        let flat_config = PrintConfig {
            print_width: usize::MAX / 2,
            ..self.config
        };
        let interner = self.interner.borrow();
        doc::print_doc_resolved(d, &flat_config, &*interner)
    }

    /// Wrap content and closing line with declaration indent depth handling
    ///
    /// In multi-declarator contexts (declaration_indent_depth > 0), content gets
    /// double-indented and the closing line gets single extra indent. This creates
    /// the proper visual alignment for:
    /// ```javascript
    /// const a = {
    ///         prop: value,
    ///     },
    ///     b = 2;
    /// ```
    pub(crate) fn wrap_with_decl_indent(&self, inner: Doc, closing_line: Doc) -> (Doc, Doc) {
        if self.declaration_indent_depth.get() > 0 {
            (doc::indent(doc::indent(inner)), doc::indent(closing_line))
        } else {
            (doc::indent(inner), closing_line)
        }
    }

    /// Build a Doc for an expression (public wrapper for doc-based formatting)
    ///
    /// This returns a Doc tree that can be used for line wrapping decisions
    /// when embedding TS expressions in larger documents (e.g., Svelte attributes).
    pub fn build_expression_doc_public(&self, expr: &internal::Expression) -> Doc {
        self.build_expression_doc(expr)
    }

    /// Build a Doc for a condition expression (if/while/for test) in an isolated context.
    ///
    /// For binary expressions, uses an ungrouped version so the parent group controls
    /// whether the condition breaks to multiple lines. This matches Prettier's behavior
    /// where all operands break together when the condition exceeds print width.
    ///
    /// Used for Svelte block conditions ({#if}, {:else if}) where the surrounding
    /// Svelte syntax provides the grouping context.
    pub fn build_condition_doc_public(&self, expr: &internal::Expression) -> Doc {
        match expr {
            internal::Expression::BinaryExpression(binary) => {
                // Use ungrouped version so parent group controls breaking
                self.build_binary_chain_doc_ungrouped(binary)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build a Doc for an expression with continuation indent for binary expressions.
    ///
    /// For binary expressions, produces:
    /// ```text
    /// first &&
    ///   second &&
    ///   third
    /// ```
    ///
    /// Other expressions are built normally.
    pub fn build_expression_doc_with_continuation_indent_public(
        &self,
        expr: &internal::Expression,
    ) -> Doc {
        match expr {
            internal::Expression::BinaryExpression(binary) => {
                self.build_binary_chain_doc_with_continuation_indent(binary)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Check if identifier has a complex type annotation (nested generics)
    ///
    /// Corresponds to prettier's `hasComplexTypeAnnotation`:
    /// - Type reference with >1 type parameters
    /// - At least one type param has nested generics OR is a conditional type
    ///
    /// Example: `Map<string, Array<number>>` - Map has 2 params, second has nested generic
    pub(crate) fn id_has_complex_type_annotation(&self, expr: &internal::Expression) -> bool {
        let type_ann = match expr {
            internal::Expression::Identifier(id) => id.type_annotation.as_ref(),
            internal::Expression::ObjectPattern(obj) => obj.type_annotation.as_ref(),
            internal::Expression::ArrayPattern(arr) => arr.type_annotation.as_ref(),
            _ => None,
        };

        type_ann.is_some_and(|ann| self.type_has_complex_annotation(&ann.type_annotation))
    }

    /// Check if a type has complex nested type parameters
    fn type_has_complex_annotation(&self, ts_type: &internal::TSType) -> bool {
        match ts_type {
            internal::TSType::TypeReference(type_ref) => {
                // Must have >1 type argument
                let type_args = match &type_ref.type_arguments {
                    Some(args) => &args.params,
                    None => return false,
                };

                if type_args.len() <= 1 {
                    return false;
                }

                // At least one arg must have nested generics or be a conditional type
                type_args
                    .iter()
                    .any(|param| self.type_has_nested_generics(param))
            }
            _ => false,
        }
    }

    /// Check if a type has nested type parameters or is a conditional type
    fn type_has_nested_generics(&self, ts_type: &internal::TSType) -> bool {
        match ts_type {
            internal::TSType::TypeReference(type_ref) => {
                // Has type arguments means nested generics
                type_ref.type_arguments.is_some()
            }
            internal::TSType::Conditional(_) => true,
            _ => false,
        }
    }

    /// Check if a type alias has complex type parameters
    ///
    /// Corresponds to prettier's `isComplexTypeAliasParams`:
    /// - >1 type parameter
    /// - At least one has a constraint or default value
    ///
    /// Example: `type Foo<T extends string, U = number> = ...`
    pub(crate) fn type_alias_has_complex_params(
        &self,
        type_params: Option<&internal::TSTypeParameterDeclaration>,
    ) -> bool {
        let params = match type_params {
            Some(p) => &p.params,
            None => return false,
        };

        if params.len() <= 1 {
            return false;
        }

        // At least one param has a constraint or default
        params
            .iter()
            .any(|param| param.constraint.is_some() || param.default.is_some())
    }

    /// Check if identifier has complex destructuring pattern
    ///
    /// Corresponds to prettier's `isComplexDestructuring`:
    /// - ObjectPattern with >2 properties
    /// - At least one property has a default value OR is not shorthand
    ///
    /// Example: `const { a, b = 1, c } = obj` - 3 properties, one has default
    pub(crate) fn id_has_complex_destructuring(&self, expr: &internal::Expression) -> bool {
        let internal::Expression::ObjectPattern(obj) = expr else {
            return false;
        };

        if obj.properties.len() <= 2 {
            return false;
        }

        // At least one property has a default value or is not shorthand
        obj.properties.iter().any(|prop| {
            match prop {
                internal::ObjectPatternProperty::Property(p) => {
                    // Has default if value is AssignmentPattern
                    let has_default = matches!(p.value, internal::Expression::AssignmentPattern(_));
                    // Not shorthand if key != value
                    let not_shorthand = !p.shorthand;
                    has_default || not_shorthand
                }
                internal::ObjectPatternProperty::RestElement(_) => false,
            }
        })
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

    /// Find the closing `)` between a start position and end boundary.
    ///
    /// Scans the source to find the `)` that closes the params. Returns
    /// the position AFTER the `)` for use as a boundary.
    pub(crate) fn find_closing_paren(&self, start: u32, end: u32) -> Option<u32> {
        let source = self.source.as_bytes();
        let end = end as usize;
        let mut depth = 0;
        let mut i = start as usize;

        while i < end && i < source.len() {
            match source[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i + 1) as u32);
                    }
                }
                _ => {
                    if let Some(skip) = analysis::skip_string_or_comment(source, i, end) {
                        i = skip;
                    }
                }
            }
            i += 1;
        }
        None
    }

    /// Find the `=>` token between a start position and end boundary.
    ///
    /// Scans the source to find `=>`. Returns the position OF the `=` character
    /// (the start of the arrow token). Skips over comments and strings.
    pub(crate) fn find_arrow_token(&self, start: u32, end: u32) -> Option<u32> {
        let source = self.source.as_bytes();
        let end = end as usize;
        let mut i = start as usize;

        while i + 1 < end && i + 1 < source.len() {
            if source[i] == b'=' && source[i + 1] == b'>' {
                return Some(i as u32);
            }
            if let Some(skip) = analysis::skip_string_or_comment(source, i, end) {
                i = skip;
            }
            i += 1;
        }
        None
    }

    /// Find a keyword between a start position and end boundary.
    ///
    /// Returns the position of the first character of the keyword if found.
    /// Skips over comments and strings. Checks for word boundaries (keyword
    /// must not be part of a larger identifier).
    pub(crate) fn find_keyword_in_range(&self, start: u32, end: u32, keyword: &str) -> Option<u32> {
        let source = self.source.as_bytes();
        let kw_bytes = keyword.as_bytes();
        let end = end as usize;
        let kw_len = kw_bytes.len();
        let mut i = start as usize;

        while i + kw_len <= end && i + kw_len <= source.len() {
            // Check for keyword match
            if &source[i..i + kw_len] == kw_bytes {
                // Check word boundaries (not part of larger identifier)
                let before_ok =
                    i == 0 || !source[i - 1].is_ascii_alphanumeric() && source[i - 1] != b'_';
                let after_ok = i + kw_len >= source.len()
                    || !source[i + kw_len].is_ascii_alphanumeric() && source[i + kw_len] != b'_';
                if before_ok && after_ok {
                    return Some(i as u32);
                }
            }
            if let Some(skip) = analysis::skip_string_or_comment(source, i, end) {
                i = skip;
            }
            i += 1;
        }
        None
    }

    /// Build a Doc for a list of decorators, each on its own line
    ///
    /// Returns None if there are no decorators.
    /// Each decorator is formatted as `@expression` followed by hardline.
    pub(crate) fn build_decorators_doc(
        &self,
        decorators: Option<&Vec<internal::Decorator>>,
    ) -> Option<Doc> {
        let decorators = decorators?;
        if decorators.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for decorator in decorators {
            parts.push(doc::text("@"));
            parts.push(self.build_expression_doc(&decorator.expression));
            parts.push(doc::hardline());
        }
        Some(doc::concat(parts))
    }

    /// Print a TypeScript program
    pub fn print_program(&mut self, program: &internal::Program) {
        let mut prev_end = 0u32; // Start of file
        // Track if we've printed any output (comments or statements)
        // Used to correctly handle blank line preservation - we never want blank lines
        // at the very start of the program, but we do want them between subsequent items.
        let mut has_output = false;

        // Filter out standalone EmptyStatements - prettier removes them
        // (EmptyStatement is still printed when part of control flow: if/while/for/label)
        for statement in program.body.iter() {
            // Skip standalone empty statements but preserve blank lines around them
            if matches!(statement, internal::Statement::EmptyStatement(_)) {
                // Add separator only if buffer doesn't end with newline
                // (statements don't add trailing newlines, but comments do)
                // This ensures we start on a new line for blank line detection to work correctly.
                if has_output && !self.buffer.ends_with('\n') {
                    self.write("\n");
                }
                // Process comments associated with this statement (preserves blank lines within)
                let printed_comments =
                    self.print_leading_comments(prev_end, statement.span().start, !has_output);
                if printed_comments {
                    has_output = true;
                }
                // Remove trailing space if any (same-line comments add space for the next element,
                // but we're skipping this statement so there's no next element)
                self.buffer.pop_if_ends_with(' ');
                prev_end = statement.span().end;
                continue;
            }

            // Always add newline between output (separator)
            if has_output {
                self.write("\n");

                // Check for blank lines between statements (preserve from source)
                // Only check if there are no comments between statements
                // (comments handle their own blank line preservation)
                let stmt_start = statement.span().start;
                if !has_comments_in_range(self.comments, prev_end, stmt_start)
                    && printing::has_blank_line_between(self.source, prev_end, stmt_start)
                {
                    self.write("\n");
                }
            }

            // Print leading comments before this statement
            // (blank line preservation handled inside print_leading_comments)
            // For the first statement, include same-line comments (no previous statement to be trailing from)
            let is_first = !has_output;
            self.print_leading_comments(prev_end, statement.span().start, is_first);

            let doc = self.build_statement_doc(statement);
            self.write_doc(&doc);

            // Print trailing same-line comments after the statement
            self.print_trailing_same_line_comments(statement.span().end);

            prev_end = statement.span().end;
            has_output = true;
        }

        // Print trailing comments after the last statement
        self.print_program_trailing_comments(prev_end);

        // Add trailing newline (matches prettier)
        self.write("\n");
    }

    /// Print comments after the last statement in the program
    ///
    /// These are comments that appear after all statements but before end of file.
    /// Note: Same-line inline comments are already handled by statement-level printers
    /// (e.g., print_inline_comments_in_statement), so we skip those here.
    fn print_program_trailing_comments(&mut self, prev_end: u32) {
        let mut last_comment_end = prev_end;
        let mut is_first_comment = true;

        for comment in comments_after(self.comments, prev_end) {
            // Skip comments on same line as prev_end - those are inline trailing comments
            // already handled by statement-level printers (print_inline_comments_in_statement)
            // BUT: When prev_end == 0 (no statements), there's no previous statement to be
            // trailing from, so comments at position 0 should NOT be skipped.
            if prev_end > 0 && printing::is_same_line(self.source, prev_end, comment.span.start) {
                last_comment_end = comment.span.end;
                continue;
            }

            // For comments-only files (no statements), don't add leading newline for first comment
            if prev_end > 0 || !is_first_comment {
                // Comment on its own line - add newline and possible blank line
                self.write("\n");

                if printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                ) {
                    self.write("\n");
                }
            }

            self.write_indent();
            self.print_comment(comment);
            last_comment_end = comment.span.end;
            is_first_comment = false;
        }
    }
}

/// Check if a doc is effectively empty (empty text or empty concat)
fn is_empty_doc(d: &Doc) -> bool {
    match d {
        Doc::Text(text) => text.try_as_str().is_some_and(str::is_empty),
        Doc::Concat(docs) => docs.is_empty() || docs.iter().all(is_empty_doc),
        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Build a Doc tree for a TypeScript program
    ///
    /// Returns a Doc that can be wrapped with `indent()` and rendered.
    /// Used when embedding TypeScript in other formats like Svelte's `<script>`.
    ///
    /// The Doc structure preserves:
    /// - Statement separation with hardline
    /// - Blank line preservation between statements using literalline
    /// - Leading comments with proper spacing
    /// - Trailing same-line comments using line_suffix
    /// - Program trailing comments after the last statement
    pub fn build_program_doc(&self, program: &internal::Program) -> Doc {
        let mut parts = Vec::new();
        let mut prev_end = 0u32;
        let mut has_output = false;

        for statement in program.body.iter() {
            // Skip standalone EmptyStatements but preserve blank lines and comments around them
            if matches!(statement, internal::Statement::EmptyStatement(_)) {
                // Force non-inline: since we're skipping the semicolon, any "inline" comments
                // (on same line as the semicolon) have nothing to be inline with
                let comments_doc =
                    self.build_leading_comments_doc(prev_end, statement.span().end, !has_output, true);
                if !is_empty_doc(&comments_doc) {
                    if has_output {
                        // Check for blank line before the first comment (same as regular statements)
                        let first_comment_start = comments_in_range(self.comments, prev_end, statement.span().end)
                            .next()
                            .map(|c| c.span.start);
                        let check_end = first_comment_start.unwrap_or_else(|| statement.span().end);

                        if printing::has_blank_line_between(self.source, prev_end, check_end) {
                            parts.push(doc::literalline()); // Blank line at column 0
                        }
                        parts.push(doc::hardline()); // Separator with indent
                    }
                    parts.push(comments_doc);
                    has_output = true;
                }
                prev_end = statement.span().end;
                continue;
            }

            // Separator between statements
            if has_output {
                // Check for blank line before the next item:
                // - If there are comments, check before the first comment
                // - If no comments, check before the statement
                let first_comment_start = comments_in_range(self.comments, prev_end, statement.span().start)
                    .next()
                    .map(|c| c.span.start);
                let check_end = first_comment_start.unwrap_or_else(|| statement.span().start);

                if printing::has_blank_line_between(self.source, prev_end, check_end) {
                    parts.push(doc::literalline()); // Blank line at column 0
                }

                parts.push(doc::hardline()); // Separator with indent
            }

            // Leading comments (allow inline comments since statement will be printed)
            let leading_doc =
                self.build_leading_comments_doc(prev_end, statement.span().start, !has_output, false);
            if !is_empty_doc(&leading_doc) {
                parts.push(leading_doc);
            }

            // Statement
            parts.push(self.build_statement_doc(statement));

            // Trailing same-line comments
            let trailing_docs = self.build_trailing_same_line_comments_doc(statement.span().end);
            parts.extend(trailing_docs);

            // Update prev_end to be after any trailing same-line comments
            // This ensures blank line detection works correctly
            prev_end = self.find_end_with_trailing_comments(statement.span().end);
            has_output = true;
        }

        // Trailing program comments
        let trailing_comments_doc = self.build_program_trailing_comments_doc(prev_end);
        parts.extend(trailing_comments_doc);

        // Trailing newline
        parts.push(doc::hardline());

        doc::concat(parts)
    }

    /// Build doc for leading comments between prev_end and curr_start
    ///
    /// Returns a Doc containing all leading comments with proper blank line handling.
    /// Returns empty doc if no comments.
    ///
    /// Structure: Each comment is output WITHOUT a trailing hardline.
    /// Separators (hardline or literalline+hardline) are added BEFORE each subsequent
    /// comment and AFTER the last comment (to separate from the statement).
    ///
    /// When `force_non_inline` is true, all comments are treated as non-inline (own line).
    /// This is used for empty statements that will be skipped - their inline comments
    /// have nothing to be inline with.
    fn build_leading_comments_doc(
        &self,
        prev_end: u32,
        curr_start: u32,
        is_first: bool,
        force_non_inline: bool,
    ) -> Doc {
        let mut parts = Vec::new();
        let mut last_comment_end = prev_end;
        let mut printed_any = false;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            let position = classify_comment(comment, prev_end, curr_start, self.source);

            // Skip trailing comments EXCEPT for first statement (file start)
            if !is_first && matches!(position, CommentPosition::Trailing) {
                last_comment_end = comment.span.end;
                continue;
            }

            // Handle inline leading comments (same line as statement)
            // These stay on the same line, so DON'T set printed_any (no separator needed)
            // Skip this behavior when force_non_inline is true (e.g., empty statements being skipped)
            if !force_non_inline && matches!(position, CommentPosition::LeadingInline) {
                parts.push(self.build_comment_doc(comment));
                parts.push(doc::text(" "));
                // DON'T set printed_any - inline comments don't need separators
                last_comment_end = comment.span.end;
                continue;
            }

            // Comment on its own line: check for blank lines BETWEEN comments
            // Note: blank line before FIRST comment is handled by the parent (build_program_doc)
            // We only handle blank lines between subsequent comments here
            let has_blank_before = printed_any
                && comment.span.start > last_comment_end
                && printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                );

            // Add separator BEFORE this comment (first comment has no separator - parent's hardline handles it)
            if has_blank_before {
                parts.push(doc::literalline()); // Blank line at column 0
                parts.push(doc::hardline()); // Indent for this comment
            } else if printed_any {
                parts.push(doc::hardline()); // Separator from previous comment
            }

            parts.push(self.build_comment_doc(comment));
            // NO hardline after comment - let post-loop or next iteration handle it

            last_comment_end = comment.span.end;
            printed_any = true;
        }

        // After all comments: add separator for the statement (if one follows)
        // Skip this when force_non_inline is true - that means the statement is being skipped
        // and there's nothing for the separator to separate from
        if printed_any && !force_non_inline {
            // Check if there's a blank line after the last comment
            let has_blank_after = last_comment_end < curr_start
                && printing::has_blank_line_between(self.source, last_comment_end, curr_start);

            if has_blank_after {
                parts.push(doc::literalline()); // Blank line at column 0
            }
            parts.push(doc::hardline()); // Indent for statement
        }

        doc::concat(parts)
    }

    /// Build docs for trailing same-line comments after a node
    ///
    /// Returns a Vec of docs to append to the current parts.
    fn build_trailing_same_line_comments_doc(&self, after_pos: u32) -> Vec<Doc> {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, after_pos);
        let mut docs = Vec::new();

        for comment in &self.comments[first_idx..] {
            if printing::is_same_line(self.source, after_pos, comment.span.start) {
                if comment.is_block {
                    // Block comments are inline, affect width
                    docs.push(doc::text(" "));
                    docs.push(self.build_comment_doc(comment));
                } else {
                    // Line comments go in line_suffix, don't affect width
                    docs.push(doc::line_suffix(doc::concat(vec![
                        doc::text(" "),
                        self.build_comment_doc(comment),
                    ])));
                }
            } else {
                break;
            }
        }
        docs
    }

    /// Find the end position including any trailing same-line comments
    ///
    /// Used to correctly detect blank lines - need to check from after trailing
    /// comments, not just after the statement.
    fn find_end_with_trailing_comments(&self, after_pos: u32) -> u32 {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, after_pos);
        let mut end = after_pos;

        for comment in &self.comments[first_idx..] {
            if printing::is_same_line(self.source, after_pos, comment.span.start) {
                end = comment.span.end;
            } else {
                break;
            }
        }
        end
    }

    /// Build docs for trailing comments at the end of the program
    ///
    /// Handles comments that appear after all statements but before end of file.
    fn build_program_trailing_comments_doc(&self, prev_end: u32) -> Vec<Doc> {
        let mut docs = Vec::new();
        let mut last_comment_end = prev_end;
        let mut is_first_comment = true;

        for comment in comments_after(self.comments, prev_end) {
            // Skip comments on same line as prev_end - those are inline trailing comments
            // already handled by build_trailing_same_line_comments_doc
            // BUT: When prev_end == 0 (no statements), there's no previous statement to be
            // trailing from, so comments at position 0 should NOT be skipped.
            if prev_end > 0 && printing::is_same_line(self.source, prev_end, comment.span.start) {
                last_comment_end = comment.span.end;
                continue;
            }

            // For comments-only files (no statements), don't add leading newline for first comment
            if prev_end > 0 || !is_first_comment {
                // Blank line before this comment (add literalline BEFORE hardline)
                if printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                ) {
                    docs.push(doc::literalline());
                }

                docs.push(doc::hardline());
            }

            docs.push(self.build_comment_doc(comment));
            last_comment_end = comment.span.end;
            is_first_comment = false;
        }

        docs
    }
}

// Implement SymbolResolver trait for shared symbol resolution utilities
impl<'a> SymbolResolver for Printer<'a> {
    fn interner(&self) -> &Rc<RefCell<DefaultStringInterner>> {
        &self.interner
    }
}
