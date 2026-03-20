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

use analysis::needs_isolation_for_hugging;
pub(crate) use analysis::{
    PatternContext, build_entity_name_doc, conditional_should_break_after_op,
    has_multiline_content, has_newline_before_position, is_brace_block_multiline,
    is_module_path_fluid_call, is_multiline_string_literal, is_multiline_template_expression,
    is_pure_property_chain, is_string_literal, object_pattern_should_expand,
    template_literal_has_newlines,
};
pub(crate) use assignment::{
    arrow_chain_has_return_type, is_call_on_member_chain, is_curried_arrow_with_return_type,
    is_literal_member_chain, is_poorly_breakable_chain, is_regex_root_chain,
    is_self_expanding_value, is_simple_self_expanding, is_simple_value,
    is_single_call_on_member_chain, is_type_assertion_call, should_inline_logical_expression,
};
pub(crate) use comments::{CommentFilter, CommentSpacing};
pub(crate) use needs_parens::{ParenContext, needs_parens};
pub(crate) use types::{
    intersection_has_expanding_first_type, intersection_has_huggable_last_type,
    should_hug_union_type, unwrap_parenthesized,
};

use crate::ast::internal;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use string_interner::DefaultStringInterner;
use tsv_lang::{
    CommentPosition, OutputBuffer, PrintConfig, SymbolResolver, classify_comment_fast,
    comments_after, comments_in_range,
    doc::{
        self,
        arena::{DocArena, DocId},
    },
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
    /// Arena allocator for doc nodes (borrowed from caller or locally owned)
    pub(crate) arena: &'a DocArena,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
    /// Original source code (for extracting raw values, preserving escape sequences, etc.)
    pub(crate) source: &'a str,
    /// Comments from the program (for printing leading/trailing comments)
    pub(crate) comments: &'a [internal::Comment],
    /// Precomputed line break positions for O(log n) line boundary lookups
    pub(crate) line_breaks: &'a [u32],
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
    /// Whether we're inside a curried arrow function with return type.
    /// When true, nested arrows always break after `=>` regardless of their own return type.
    /// Used for: const f = (x: T): H => (y) => expr - ALL arrows break, not just the typed ones.
    pub(crate) in_curried_typed_arrow: Cell<bool>,
    /// Whether the next ObjectExpression should be wrapped in parens.
    /// Set when printing arrow body with chained as/satisfies wrapping an object:
    /// `() => ({}) as unknown as Logger` — parens go around inner object only.
    pub(crate) arrow_body_object_needs_parens: Cell<bool>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given arena, interner, source, comments, line_breaks, and config
    pub fn with_config(
        arena: &'a DocArena,
        interner: Rc<RefCell<DefaultStringInterner>>,
        source: &'a str,
        comments: &'a [internal::Comment],
        line_breaks: &'a [u32],
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            config,
            arena,
            interner,
            source,
            comments,
            line_breaks,
            declaration_indent_depth: Cell::new(0),
            is_expression_statement: Cell::new(false),
            in_top_level_assignment: Cell::new(false),
            force_chain_expand: Cell::new(false),
            in_template_interpolation: Cell::new(false),
            in_curried_typed_arrow: Cell::new(false),
            arrow_body_object_needs_parens: Cell::new(false),
        }
    }

    /// Get a reference to the doc arena.
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        self.arena
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write a DocId to the buffer, accounting for current column and indent level
    ///
    /// This handles the common pattern of:
    /// 1. Calculate current column with context offset
    /// 2. Print doc with indent-aware width calculations
    /// 3. Write the result to the buffer
    ///
    /// For width calculations, we account for outer context in two ways:
    /// - If `first_line_offset > 0`: expression is embedded inline (e.g., Svelte block), use it directly
    /// - If `first_line_offset == 0`: standalone block (e.g., `<script>`), use `base_indent_offset * tab_width`
    pub(crate) fn write_arena_doc(&mut self, d: DocId) {
        let context_offset = if self.config.first_line_offset > 0 {
            if self.current_column() == 0 {
                self.config.first_line_offset
            } else {
                0
            }
        } else {
            self.config.base_indent_offset * self.config.tab_width
        };
        let current_col = self.current_column() + context_offset;
        let output = {
            let interner = self.interner.borrow();
            doc::arena_print_doc_with_indent_resolved(
                self.arena,
                d,
                &self.config,
                current_col,
                self.indent_level,
                &*interner,
            )
        };
        self.write(&output);
    }

    /// Render an arena DocId to a flat string with effectively infinite width.
    pub(crate) fn render_arena_doc_flat(&self, d: DocId) -> String {
        let flat_config = PrintConfig {
            print_width: usize::MAX / 2,
            ..self.config
        };
        let interner = self.interner.borrow();
        doc::arena_print_doc_resolved(self.arena, d, &flat_config, &*interner)
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

    /// Compute the visual indent width at a source position.
    ///
    /// Finds the start of the line containing `pos` and measures the leading
    /// whitespace visual width (tabs count as `tab_width` chars).
    pub(crate) fn source_indent_visual(&self, pos: u32) -> usize {
        let pos = pos as usize;
        let line_start = self.source[..pos].rfind('\n').map_or(0, |i| i + 1);
        printing::visual_width(&self.source[line_start..pos], self.config.tab_width)
    }

    /// Check if two positions are on the same line (O(log n) binary search)
    #[inline]
    pub(crate) fn is_same_line(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::is_same_line_fast(self.line_breaks, prev_end, curr_start)
    }

    /// Check if there's a blank line (2+ newlines) between two positions (O(log n) binary search)
    #[inline]
    pub(crate) fn has_blank_line_between(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::has_blank_line_between_fast(self.line_breaks, prev_end, curr_start)
    }

    /// Check if there's any newline between two positions (O(log n) binary search)
    #[inline]
    pub(crate) fn has_newline_between(&self, start: u32, end: u32) -> bool {
        printing::has_newline_between_fast(self.line_breaks, start, end)
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
    pub(crate) fn wrap_with_decl_indent(
        &self,
        inner: DocId,
        closing_line: DocId,
    ) -> (DocId, DocId) {
        let d = self.d();
        if self.declaration_indent_depth.get() > 0 {
            (d.indent(d.indent(inner)), d.indent(closing_line))
        } else {
            (d.indent(inner), closing_line)
        }
    }

    /// Build a DocId for an expression (public wrapper for doc-based formatting)
    ///
    /// This returns a DocId that can be used for line wrapping decisions
    /// when embedding TS expressions in larger documents (e.g., Svelte attributes).
    pub fn build_expression_doc_public(&self, expr: &internal::Expression) -> DocId {
        self.build_expression_doc(expr)
    }

    /// Build expression doc with IsolatedGroup wrapping for huggable expressions.
    ///
    /// Wraps templates and arrow-with-template-body in `isolated_group` to prevent
    /// internal breaks from forcing parent calls/arrays to break (enables hugging).
    pub(crate) fn build_huggable_expression_doc(&self, expr: &internal::Expression) -> DocId {
        let d = self.d();
        let base_doc = self.build_arg_expression_doc(expr);
        if needs_isolation_for_hugging(expr) {
            d.isolated_group(base_doc)
        } else {
            base_doc
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
    /// Skips over comments to avoid matching `=` inside them.
    /// Also skips `==` and `===` comparison operators (we want assignment `=`).
    pub(crate) fn find_equals_position(&self, start: u32, end: u32) -> u32 {
        let bytes = self.source.as_bytes();
        let start_pos = start as usize;
        let end_pos = end as usize;
        let mut i = start_pos;

        while i < end_pos {
            if let Some(new_i) = analysis::skip_comment(bytes, i, end_pos) {
                i = new_i;
                continue;
            }
            // Check for assignment `=` (not `==` or `===`)
            if bytes[i] == b'=' && (i + 1 >= end_pos || bytes[i + 1] != b'=') {
                return i as u32;
            }
            i += 1;
        }
        // Fallback: return midpoint if `=` not found
        usize::midpoint(start_pos, end_pos) as u32
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

    /// Check if there are multiline block comments between two positions
    ///
    /// Multiline block comments (containing newlines) force break-after-operator
    /// layout in assignments and property values.
    /// Prettier ref: `hasLeadingOwnLineComment` in assignment.js `chooseLayout`
    pub(crate) fn has_multiline_block_comments_between(&self, start: u32, end: u32) -> bool {
        tsv_lang::has_multiline_block_comments_in_range(self.comments, start, end)
    }

    /// Check if a delimited list (tuple, type params, etc.) has line comments
    /// between any elements OR after the last element.
    ///
    /// Used to determine if a list should be forced to multiline formatting.
    pub(crate) fn has_line_comments_in_delimited_list<T, F>(
        &self,
        items: &[T],
        get_span: F,
        end_boundary: u32,
    ) -> bool
    where
        F: Fn(&T) -> tsv_lang::Span,
    {
        let between = items.windows(2).any(|pair| {
            self.has_line_comments_between(get_span(&pair[0]).end, get_span(&pair[1]).start)
        });
        let trailing = items
            .last()
            .is_some_and(|last| self.has_line_comments_between(get_span(last).end, end_boundary));
        between || trailing
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

    /// Find the `=>` token position for an arrow function.
    ///
    /// Computes the signature end from the arrow's structure and scans for `=>`.
    /// Returns the position of `=` in `=>`, or the body start as fallback.
    pub(crate) fn find_arrow_token_for(&self, arrow: &internal::ArrowFunctionExpression) -> u32 {
        let body_start = arrow.body.span().start;
        let sig_end = if let Some(rt) = &arrow.return_type {
            rt.span.end
        } else if let Some(ps) = arrow.params_start {
            self.find_closing_paren(ps, body_start)
                .unwrap_or(body_start)
        } else {
            arrow
                .params
                .last()
                .map_or(arrow.span.start, |p| p.span().end)
        };
        self.find_arrow_token(sig_end, body_start)
            .unwrap_or(body_start)
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
    ) -> Option<DocId> {
        let decorators = decorators?;
        if decorators.is_empty() {
            return None;
        }
        let d = self.d();
        let mut parts = Vec::new();
        for decorator in decorators {
            parts.push(d.text("@"));
            parts.push(self.build_expression_doc(&decorator.expression));
            parts.push(d.hardline());
        }
        Some(d.concat(&parts))
    }

    /// Print a TypeScript program
    ///
    /// Delegates to `build_program_doc` to build the doc tree, then renders it.
    /// This is the same path used by Svelte's `<script>` formatting, ensuring
    /// consistent behavior (e.g., trailing whitespace trimming in comments).
    pub fn print_program(&mut self, program: &internal::Program) {
        let doc = self.build_program_doc(program);
        self.write_arena_doc(doc);
    }
}

impl<'a> Printer<'a> {
    /// Build a DocId tree for a TypeScript program
    ///
    /// Returns a DocId that can be wrapped with `indent()` and rendered.
    /// Used both for standalone TS/JS formatting (via `print_program`) and
    /// when embedding TypeScript in other formats like Svelte's `<script>`.
    ///
    /// The Doc structure preserves:
    /// - Statement separation with hardline
    /// - Blank line preservation between statements using literalline
    /// - Leading comments with proper spacing
    /// - Trailing same-line comments using line_suffix
    /// - Program trailing comments after the last statement
    pub fn build_program_doc(&self, program: &internal::Program) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end = 0u32;
        let mut has_output = false;

        for statement in program.body.iter() {
            // Skip standalone EmptyStatements but preserve blank lines and comments around them
            if matches!(statement, internal::Statement::EmptyStatement(_)) {
                // Force non-inline: since we're skipping the semicolon, any "inline" comments
                // (on same line as the semicolon) have nothing to be inline with
                let comments_doc = self.build_leading_comments_doc(
                    prev_end,
                    statement.span().end,
                    !has_output,
                    true,
                );
                if let Some(comments_doc) = comments_doc {
                    if has_output {
                        // Check for blank line before the first comment (same as regular statements)
                        let first_comment_start =
                            comments_in_range(self.comments, prev_end, statement.span().end)
                                .next()
                                .map(|c| c.span.start);
                        let check_end = first_comment_start.unwrap_or_else(|| statement.span().end);

                        if self.has_blank_line_between(prev_end, check_end) {
                            parts.push(d.literalline()); // Blank line at column 0
                        }
                        parts.push(d.hardline()); // Separator with indent
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
                let first_comment_start =
                    comments_in_range(self.comments, prev_end, statement.span().start)
                        .next()
                        .map(|c| c.span.start);
                let check_end = first_comment_start.unwrap_or_else(|| statement.span().start);

                if self.has_blank_line_between(prev_end, check_end) {
                    parts.push(d.literalline()); // Blank line at column 0
                }

                parts.push(d.hardline()); // Separator with indent
            }

            // Leading comments (allow inline comments since statement will be printed)
            if let Some(leading_doc) = self.build_leading_comments_doc(
                prev_end,
                statement.span().start,
                !has_output,
                false,
            ) {
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
        parts.push(d.hardline());

        d.concat(&parts)
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
    ) -> Option<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        let mut last_comment_end = prev_end;
        let mut printed_any = false;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            let position = classify_comment_fast(comment, prev_end, curr_start, self.line_breaks);

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
                parts.push(d.text(" "));
                // DON'T set printed_any - inline comments don't need separators
                last_comment_end = comment.span.end;
                continue;
            }

            // Comment on its own line: check for blank lines BETWEEN comments
            // Note: blank line before FIRST comment is handled by the parent (build_program_doc)
            // We only handle blank lines between subsequent comments here
            let has_blank_before = printed_any
                && comment.span.start > last_comment_end
                && self.has_blank_line_between(last_comment_end, comment.span.start);

            // Add separator BEFORE this comment (first comment has no separator - parent's hardline handles it)
            if has_blank_before {
                parts.push(d.literalline()); // Blank line at column 0
                parts.push(d.hardline()); // Indent for this comment
            } else if printed_any {
                parts.push(d.hardline()); // Separator from previous comment
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
                && self.has_blank_line_between(last_comment_end, curr_start);

            if has_blank_after {
                parts.push(d.literalline()); // Blank line at column 0
            }
            parts.push(d.hardline()); // Indent for statement
        }

        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
        }
    }

    /// Build docs for trailing same-line comments after a node
    ///
    /// Returns a Vec of docs to append to the current parts.
    fn build_trailing_same_line_comments_doc(&self, after_pos: u32) -> Vec<DocId> {
        let d = self.d();
        let first_idx = tsv_lang::find_first_comment_from(self.comments, after_pos);
        let mut docs = Vec::new();

        for comment in &self.comments[first_idx..] {
            if self.is_same_line(after_pos, comment.span.start) {
                if comment.is_block {
                    // Block comments are inline, affect width
                    docs.push(d.text(" "));
                    docs.push(self.build_comment_doc(comment));
                } else {
                    // Line comments go in line_suffix, don't affect width
                    docs.push(self.build_trailing_line_comment_doc(comment));
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
            if self.is_same_line(after_pos, comment.span.start) {
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
    fn build_program_trailing_comments_doc(&self, prev_end: u32) -> Vec<DocId> {
        let d = self.d();
        let mut docs = Vec::new();
        let mut last_comment_end = prev_end;
        let mut is_first_comment = true;

        for comment in comments_after(self.comments, prev_end) {
            // Skip comments on same line as prev_end - those are inline trailing comments
            // already handled by build_trailing_same_line_comments_doc
            // BUT: When prev_end == 0 (no statements), there's no previous statement to be
            // trailing from, so comments at position 0 should NOT be skipped.
            if prev_end > 0 && self.is_same_line(prev_end, comment.span.start) {
                last_comment_end = comment.span.end;
                continue;
            }

            // For comments-only files (no statements), don't add leading newline for first comment
            if prev_end > 0 || !is_first_comment {
                // Blank line before this comment (add literalline BEFORE hardline)
                if self.has_blank_line_between(last_comment_end, comment.span.start) {
                    docs.push(d.literalline());
                }

                docs.push(d.hardline());
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
