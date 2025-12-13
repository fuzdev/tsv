// TypeScript printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct and program printing orchestration
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

mod arrays;
mod assignment;
mod calls;
mod chain;
mod expression_stringifier;
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
    OutputBuffer, PrintConfig, SymbolResolver, comments_after, comments_in_range,
    doc::{self, Doc},
    has_comments_in_range, has_line_comments_in_range, printing,
};

/// Spacing style for comments in doc building
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentSpacing {
    /// Space before comment: ` /* c */`
    Leading,
    /// Space after comment: `/* c */ `
    Trailing,
    /// No spacing: `/* c */`
    None,
}

/// Filter for which comment types to include
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentFilter {
    /// Include all comments (block and line)
    All,
    /// Only include block comments (/* */)
    BlockOnly,
}

/// Get the content bounds inside a delimited span (e.g., `[...]`, `{...}`, `(...)`)
///
/// For a span covering `[a, b, c]`:
/// - `span.start` points to `[`
/// - `span.end` points to position after `]`
/// - Returns `(span.start + 1, span.end - 1)` = positions of content area
///
/// Useful for finding comments inside containers when the content might be empty.
#[inline]
pub(crate) fn content_bounds(span: tsv_lang::Span) -> (u32, u32) {
    (span.start + 1, span.end - 1)
}

/// Check if an expression contains a logical binary expression at its core
///
/// Used to detect expressions like `!!(a || b)` or `new (a || b)()` that need
/// doc-based width wrapping for the logical chain inside.
pub(crate) fn contains_logical_expression(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::BinaryExpression(binary) => {
            matches!(
                binary.operator,
                internal::BinaryOperator::PipePipe
                    | internal::BinaryOperator::AmpersandAmpersand
                    | internal::BinaryOperator::QuestionQuestion
            )
        }
        internal::Expression::UnaryExpression(unary) => {
            contains_logical_expression(&unary.argument)
        }
        internal::Expression::NewExpression(new_expr) => {
            contains_logical_expression(&new_expr.callee)
        }
        _ => false,
    }
}

/// Check if an expression needs doc-based printing for width-based wrapping
///
/// Expressions that need doc-based printing include:
/// - Unary expressions containing logical expressions (like `!!(a || b)`)
/// - New expressions with logical callees (like `new (a || b)()`)
///
/// These expressions have internal groups that need the doc system to
/// evaluate whether they should break based on line width.
pub(crate) fn needs_doc_based_wrapping(expr: &internal::Expression) -> bool {
    match expr {
        // Unary with logical inside: !(a || b), !!(a || b)
        internal::Expression::UnaryExpression(unary) => {
            contains_logical_expression(&unary.argument)
        }
        // New with logical callee: new (a || b)()
        internal::Expression::NewExpression(new_expr) => {
            // Check if callee needs parens AND is a logical expression
            matches!(
                new_expr.callee.as_ref(),
                internal::Expression::BinaryExpression(_)
                    | internal::Expression::ConditionalExpression(_)
                    | internal::Expression::AssignmentExpression(_)
                    | internal::Expression::SequenceExpression(_)
            ) && contains_logical_expression(&new_expr.callee)
        }
        _ => false,
    }
}

/// Check if an expression is a module path call that should use fluid assignment wrapping
/// (break after `=` if too long, keeping the call together).
///
/// Patterns:
/// - `require.resolve(stringLiteral)`
/// - `await import(stringLiteral)`
pub(crate) fn is_module_path_fluid_call(
    expr: &internal::Expression,
    interner: &DefaultStringInterner,
) -> bool {
    // Check for `await import(string)`
    if let internal::Expression::AwaitExpression(await_expr) = expr
        && let internal::Expression::ImportExpression(import_expr) = await_expr.argument.as_ref()
    {
        let is_string_arg = matches!(
            import_expr.source.as_ref(),
            internal::Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. })
        );
        return is_string_arg;
    }

    let internal::Expression::CallExpression(call) = expr else {
        return false;
    };

    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 {
        return false;
    }
    let is_string_arg = matches!(
        &call.arguments[0],
        internal::Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. })
    );
    if !is_string_arg {
        return false;
    }

    // Check for `require.resolve()`
    if let internal::Expression::MemberExpression(member) = call.callee.as_ref()
        && !member.computed
        && !member.optional
        && let internal::Expression::Identifier(resolve_id) = member.property.as_ref()
        && interner.resolve(resolve_id.name) == Some("resolve")
        && let internal::Expression::Identifier(require_id) = member.object.as_ref()
        && interner.resolve(require_id.name) == Some("require")
    {
        return true;
    }

    false
}

/// Check if an expression is a pure property chain (member expressions without calls)
///
/// Pure property chains like `obj.a.b.c` or `obj!.a!.b!` should use fluid assignment wrapping
/// (break after `=` if doesn't fit). Expressions containing calls, objects,
/// arrays, or ternaries handle their own wrapping internally.
pub(crate) fn is_pure_property_chain(expr: &internal::Expression) -> bool {
    match expr {
        // A member expression is a property chain if its object is also a pure chain
        internal::Expression::MemberExpression(member) => is_pure_property_chain(&member.object),
        // TSNonNullExpression is transparent - recurse through it
        internal::Expression::TSNonNullExpression(non_null) => {
            is_pure_property_chain(&non_null.expression)
        }
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

/// Check if a type literal was written as multiline in source
///
/// Detects newline immediately after opening brace: `{\n  ...}` vs `{ ... }`
/// Used for both formatting decisions and skip-fluid-layout checks.
pub(crate) fn is_type_literal_multiline(source: &str, span: tsv_lang::Span) -> bool {
    let source_text = &source[span.start as usize..span.end as usize];
    let after_brace = source_text.strip_prefix('{').unwrap_or("");
    after_brace.starts_with('\n')
        || after_brace.starts_with("\r\n")
        || after_brace.trim_start_matches(' ').starts_with('\n')
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
        // Private identifiers are just #name, no multiline content
        internal::Expression::PrivateIdentifier(_) => false,
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
        // Function and class expressions don't contribute to multiline content detection
        // They have their own block formatting
        internal::Expression::FunctionExpression(_) => false,
        internal::Expression::ClassExpression(_) => false,
        internal::Expression::AwaitExpression(await_expr) => {
            has_multiline_content(&await_expr.argument, source)
        }
        internal::Expression::YieldExpression(yield_expr) => yield_expr
            .argument
            .as_ref()
            .is_some_and(|arg| has_multiline_content(arg, source)),
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
        // Type assertion expressions: check the inner expression
        internal::Expression::TSTypeAssertion(type_assert) => {
            has_multiline_content(&type_assert.expression, source)
        }
        internal::Expression::TSAsExpression(as_expr) => {
            has_multiline_content(&as_expr.expression, source)
        }
        internal::Expression::TSSatisfiesExpression(sat_expr) => {
            has_multiline_content(&sat_expr.expression, source)
        }
        internal::Expression::TSInstantiationExpression(inst_expr) => {
            has_multiline_content(&inst_expr.expression, source)
        }
        internal::Expression::TSNonNullExpression(non_null_expr) => {
            has_multiline_content(&non_null_expr.expression, source)
        }
        internal::Expression::ImportExpression(import_expr) => {
            has_multiline_content(&import_expr.source, source)
        }
        // Meta properties (import.meta, new.target) are never multiline
        internal::Expression::MetaProperty(_) => false,
        // Parameter properties don't contain multiline content
        internal::Expression::TSParameterProperty(_) => false,
    }
}

/// Build doc for TSEntityName (qualified names like `A.B.C`)
///
/// This is a standalone function since it doesn't need printer state -
/// it only uses `doc::symbol()` for deferred symbol resolution.
pub(crate) fn build_entity_name_doc(name: &internal::TSEntityName) -> Doc {
    use string_interner::Symbol;
    use tsv_lang::doc;

    match name {
        internal::TSEntityName::Identifier(id) => doc::symbol(id.name.to_usize() as u32),
        internal::TSEntityName::QualifiedName(qn) => doc::concat(vec![
            build_entity_name_doc(&qn.left),
            doc::text("."),
            doc::symbol(qn.right.name.to_usize() as u32),
        ]),
    }
}

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

    /// Write a Doc to the buffer, accounting for current column and indent level
    ///
    /// This handles the common pattern of:
    /// 1. Calculate current column with base offset
    /// 2. Print doc with indent-aware width calculations
    /// 3. Write the result to the buffer
    ///
    /// Use `write_doc_with_margin` for expressions where trailing punctuation
    /// (like semicolons) should be considered in width calculations.
    pub(crate) fn write_doc(&mut self, d: &Doc) {
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset;
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

    /// Write a Doc to the buffer with +1 margin for trailing punctuation
    ///
    /// Use this for expressions followed by semicolons, commas, or other punctuation
    /// that should be accounted for in width calculations.
    pub(crate) fn write_doc_with_margin(&mut self, d: &Doc) {
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1;
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
        if self.declaration_indent_depth > 0 {
            (doc::indent(doc::indent(inner)), doc::indent(closing_line))
        } else {
            (doc::indent(inner), closing_line)
        }
    }

    /// Get the total indent increment for container contents (1 + declaration depth)
    pub(crate) fn container_indent_increment(&self) -> usize {
        1 + self.declaration_indent_depth
    }

    /// Write closing indent for container (handles declaration_indent_depth)
    ///
    /// Decrements indent by 1, writes the indent, then decrements by declaration_indent_depth.
    /// Used before writing closing bracket/brace.
    pub(crate) fn write_container_closing_indent(&mut self) {
        self.indent_level -= 1;
        self.write_indent();
        self.indent_level -= self.declaration_indent_depth;
    }

    /// Print an empty container (object or array) that contains only comments
    ///
    /// Common pattern for `{/* comment */}` → `{\n\t/* comment */\n}`
    pub(crate) fn print_empty_container_with_comments(
        &mut self,
        open: &str,
        close: &str,
        inner_start: u32,
        inner_end: u32,
    ) {
        self.write(open);
        self.write("\n");
        self.indent_level += self.container_indent_increment();

        // Print all comments inside the container
        for comment in comments_in_range(self.comments, inner_start, inner_end) {
            self.write_indent();
            self.print_comment(comment);
            self.write("\n");
        }

        self.write_container_closing_indent();
        self.write(close);
    }

    /// Build a Doc for an expression (public wrapper for doc-based formatting)
    ///
    /// This returns a Doc tree that can be used for line wrapping decisions
    /// when embedding TS expressions in larger documents (e.g., Svelte attributes).
    pub fn build_expression_doc_public(&self, expr: &internal::Expression) -> Doc {
        self.build_expression_doc(expr)
    }

    /// Build a Doc for an expression in an isolated context (e.g., inside `={...}`)
    ///
    /// Unlike `build_expression_doc_public`, this does NOT wrap sequence expressions
    /// in parentheses since the surrounding context provides grouping.
    pub fn build_expression_doc_isolated_public(&self, expr: &internal::Expression) -> Doc {
        // Special case: sequence expressions don't need parens in isolated context
        if let internal::Expression::SequenceExpression(seq) = expr {
            self.build_sequence_doc_bare(seq)
        } else {
            self.build_expression_doc(expr)
        }
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

    /// Check if an identifier has a multiline type annotation (type literal with newlines)
    ///
    /// Used to skip fluid layout in variable declarations when the LHS has a
    /// multiline type, since group-based wrapping would cause unwanted breaks.
    pub(crate) fn id_has_multiline_type(&self, expr: &internal::Expression) -> bool {
        // Extract type annotation from any binding pattern
        let type_ann = match expr {
            internal::Expression::Identifier(id) => id.type_annotation.as_ref(),
            internal::Expression::ObjectPattern(obj) => obj.type_annotation.as_ref(),
            internal::Expression::ArrayPattern(arr) => arr.type_annotation.as_ref(),
            _ => None,
        };
        type_ann.is_some_and(|ann| self.type_has_multiline_literal(&ann.type_annotation))
    }

    /// Check if a type contains a multiline type literal (object type with newlines)
    fn type_has_multiline_literal(&self, ts_type: &internal::TSType) -> bool {
        match ts_type {
            internal::TSType::TypeLiteral(t) => is_type_literal_multiline(self.source, t.span),
            // Recursively check nested types
            internal::TSType::Union(u) => {
                u.types.iter().any(|t| self.type_has_multiline_literal(t))
            }
            internal::TSType::Intersection(i) => {
                i.types.iter().any(|t| self.type_has_multiline_literal(t))
            }
            internal::TSType::Array(a) => self.type_has_multiline_literal(&a.element_type),
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

                // Prettier adds a space before */ when:
                // 1. There are exactly 2 lines (just newline between /* and */)
                // 2. The closing line is empty
                // For 3+ lines, there's content or blank lines to preserve, so no space is added
                let last_line = lines.last().unwrap_or(&"");
                let add_closing_space = lines.len() == 2 && last_line.is_empty();

                for (i, line) in lines.iter().enumerate() {
                    let is_last = i == lines.len() - 1;
                    if i > 0 {
                        // Add newline and indentation for subsequent lines
                        // Skip indentation for empty closing line when adding space
                        self.write("\n");
                        if !(is_last && add_closing_space) {
                            self.write_indent();
                        }
                    }
                    self.write(line);
                }

                // Add space before */ for empty closing line (prettier behavior)
                if add_closing_space {
                    self.write(" ");
                }
            } else {
                // Single-line block comment
                self.write(&comment.content);
            }

            self.write("*/");
        } else if comment.span.start == 0 && comment.content.starts_with("#!") {
            // Hashbang comment: #!/usr/bin/env node (no // prefix)
            // Content already includes the #! prefix
            self.write(&comment.content);
        } else {
            // Line comment: // content (no closing delimiter)
            self.write("//");
            self.write(&comment.content);
        }
    }

    /// Print a TypeScript program
    pub fn print_program(&mut self, program: &internal::Program) {
        let mut prev_end = 0u32; // Start of file
        let mut printed_count = 0usize;

        // Filter out standalone EmptyStatements - prettier removes them
        // (EmptyStatement is still printed when part of control flow: if/while/for/label)
        for statement in program.body.iter() {
            // Skip standalone empty statements but preserve blank lines around them
            if matches!(statement, internal::Statement::EmptyStatement(_)) {
                // Add separator newline if not first (so print_leading_comments starts on new line)
                if printed_count > 0 {
                    self.write("\n");
                }
                // Process comments associated with this statement (preserves blank lines within)
                self.print_leading_comments(prev_end, statement.span().start, printed_count == 0);
                // Remove trailing space if any (same-line comments add space for the next element,
                // but we're skipping this statement so there's no next element)
                self.buffer.pop_if_ends_with(' ');
                prev_end = statement.span().end;
                continue;
            }

            // Always add newline between statements (separator)
            if printed_count > 0 {
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
            let is_first = printed_count == 0;
            self.print_leading_comments(prev_end, statement.span().start, is_first);

            self.print_statement(statement);

            prev_end = statement.span().end;
            printed_count += 1;
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

    /// Build a Doc for inline comments between two positions with specified spacing and filter
    ///
    /// Returns a Doc containing all comments in the range with the specified spacing.
    /// Returns empty concat if no comments found.
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn build_comments_between(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
    ) -> Doc {
        self.build_comments_between_filtered(start, end, spacing, CommentFilter::All)
    }

    /// Build a Doc for inline comments with filtering
    pub(crate) fn build_comments_between_filtered(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> Doc {
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            // Apply filter
            if matches!(filter, CommentFilter::BlockOnly) && !comment.is_block {
                continue;
            }

            match spacing {
                CommentSpacing::Leading => {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(doc::text(" "));
                }
                CommentSpacing::None => {
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }
        doc::concat(parts)
    }

    /// Build a Doc for inline comments between two positions (leading space)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc(&self, start: u32, end: u32) -> Doc {
        self.build_comments_between(start, end, CommentSpacing::Leading)
    }

    /// Build a Doc for inline comments between two positions (no spaces)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_no_leading_space(
        &self,
        start: u32,
        end: u32,
    ) -> Doc {
        self.build_comments_between(start, end, CommentSpacing::None)
    }

    /// Print comments between two positions (imperative version)
    ///
    /// Directly writes to the output buffer instead of returning a Doc.
    pub(crate) fn print_comments_between_filtered(
        &mut self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) {
        for comment in comments_in_range(self.comments, start, end) {
            if matches!(filter, CommentFilter::BlockOnly) && !comment.is_block {
                continue;
            }
            match spacing {
                CommentSpacing::Leading => {
                    self.write(" ");
                    self.print_comment(comment);
                }
                CommentSpacing::Trailing => {
                    self.print_comment(comment);
                    self.write(" ");
                }
                CommentSpacing::None => {
                    self.print_comment(comment);
                }
            }
        }
    }

    /// Check if there's a newline between start position and the first comment in the range
    ///
    /// Returns true if there's at least one comment in the range and a newline
    /// exists between `start` and the first comment's start position.
    pub(crate) fn has_newline_before_comment(&self, start: u32, end: u32) -> bool {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        if let Some(comment) = self.comments.get(first_idx)
            && comment.span.end <= end
        {
            // Check if there's a newline between start and comment start
            let between = &self.source[start as usize..comment.span.start as usize];
            return between.contains('\n');
        }
        false
    }

    /// Build a Doc for a single comment
    ///
    /// For multi-line block comments, uses literalline to preserve original structure
    /// without adding indentation to continuation lines.
    pub(crate) fn build_comment_doc(&self, comment: &internal::Comment) -> Doc {
        if comment.is_block {
            // Block comment: /* content */
            if comment.content.contains('\n') {
                // Multi-line block comment - preserve original structure
                // Prettier keeps continuation lines at their original position relative to column 0
                let lines: Vec<&str> = comment.content.split('\n').collect();
                let mut line_docs = Vec::new();
                for (i, line) in lines.iter().enumerate() {
                    if i > 0 {
                        // Use literalline to avoid adding indentation
                        line_docs.push(doc::literalline());
                    }
                    if i == 0 {
                        line_docs.push(doc::text_owned(format!("/*{line}")));
                    } else {
                        line_docs.push(doc::text_owned((*line).to_string()));
                    }
                }
                line_docs.push(doc::text("*/"));
                doc::concat(line_docs)
            } else {
                // Single-line block comment
                doc::text_owned(format!("/*{}*/", comment.content))
            }
        } else {
            // Line comment: // content
            doc::text_owned(format!("//{}", comment.content))
        }
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
