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
mod needs_parens;
mod objects;
mod operators;
mod statements;
mod types;

pub(crate) use needs_parens::{ParenContext, needs_parens};

use crate::ast::internal;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use string_interner::DefaultStringInterner;
use tsv_lang::{
    CommentPosition, OutputBuffer, PrintConfig, SymbolResolver, SymbolToU32, classify_comment,
    comments_after, comments_in_range,
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

/// Skip over a string literal or comment starting at position `i`.
///
/// Returns `Some(end_pos)` where `end_pos` is the last character of the skipped
/// content (caller should then `i += 1`), or `None` if not at a string/comment.
fn skip_string_or_comment(source: &[u8], i: usize, end: usize) -> Option<usize> {
    match source[i] {
        b'"' | b'\'' | b'`' => {
            let quote = source[i];
            let mut j = i + 1;
            while j < end && source[j] != quote {
                if source[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            Some(j)
        }
        b'/' if i + 1 < end => {
            if source[i + 1] == b'/' {
                // Line comment - skip to end of line
                let mut j = i;
                while j < end && source[j] != b'\n' {
                    j += 1;
                }
                Some(j)
            } else if source[i + 1] == b'*' {
                // Block comment - skip to */
                let mut j = i + 2;
                while j + 1 < end && !(source[j] == b'*' && source[j + 1] == b'/') {
                    j += 1;
                }
                Some(j + 1)
            } else {
                None
            }
        }
        _ => None,
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

/// Check if an expression is a multiline string literal (contains line continuations).
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
        raw.contains("\\\n") || raw.contains("\\\r")
    } else {
        false
    }
}

/// Check if a type literal was written as multiline in source
///
/// Detects newline immediately after opening brace: `{\n  ...}` vs `{ ... }`
/// Used for both formatting decisions and skip-fluid-layout checks.
pub(crate) fn is_type_literal_multiline(source: &str, span: tsv_lang::Span) -> bool {
    let source_text = span.extract(source);
    let after_brace = source_text.strip_prefix('{').unwrap_or("");
    after_brace.starts_with('\n')
        || after_brace.starts_with("\r\n")
        || after_brace.trim_start_matches(' ').starts_with('\n')
}

/// Context for object pattern expansion decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatternContext {
    /// Pattern in function parameter position
    FunctionParameter,
    /// Pattern in standalone context (variable declaration, assignment)
    Standalone,
}

/// Check if an expression contains multiline string literals at any depth
/// Check if an object pattern should expand (print across multiple lines).
///
/// Prettier expands object patterns when any property has a nested pattern
/// (ObjectPattern or ArrayPattern) as its value. This is different from the
/// multiline content rule used for object expressions.
///
/// Prettier expands based on nesting depth and context:
/// - In function parameters: depth 3+ expands (e.g., {a: {b}} stays, {a: {b: {c}}} expands)
/// - In standalone contexts: depth 2+ expands (e.g., {a: {b}} expands)
pub(crate) fn object_pattern_should_expand(
    obj: &internal::ObjectPattern,
    context: PatternContext,
) -> bool {
    let depth = pattern_nesting_depth(obj);
    match context {
        PatternContext::FunctionParameter => depth >= 3,
        PatternContext::Standalone => depth >= 2,
    }
}

/// Calculate the maximum nesting depth of an object pattern
/// Depth 1 = simple pattern like {a}
/// Depth 2 = one level of nesting like {a: {b}}
/// Depth 3 = two levels of nesting like {a: {b: {c}}}
fn pattern_nesting_depth(obj: &internal::ObjectPattern) -> usize {
    let mut max_depth = 1;

    for prop in &obj.properties {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                let nested_depth = match &p.value {
                    internal::Expression::ObjectPattern(nested_obj) => {
                        1 + pattern_nesting_depth(nested_obj)
                    }
                    internal::Expression::ArrayPattern(nested_arr) => {
                        1 + array_pattern_nesting_depth(nested_arr)
                    }
                    internal::Expression::AssignmentPattern(ap) => match ap.left.as_ref() {
                        internal::Expression::ObjectPattern(nested_obj) => {
                            1 + pattern_nesting_depth(nested_obj)
                        }
                        internal::Expression::ArrayPattern(nested_arr) => {
                            1 + array_pattern_nesting_depth(nested_arr)
                        }
                        _ => 1,
                    },
                    _ => 1,
                };
                max_depth = max_depth.max(nested_depth);
            }
            internal::ObjectPatternProperty::RestElement(_) => {}
        }
    }

    max_depth
}

/// Calculate the maximum nesting depth of an array pattern
fn array_pattern_nesting_depth(arr: &internal::ArrayPattern) -> usize {
    let mut max_depth = 1;

    for elem in arr.elements.iter().flatten() {
        let nested_depth = match elem {
            internal::Expression::ObjectPattern(nested_obj) => {
                1 + pattern_nesting_depth(nested_obj)
            }
            internal::Expression::ArrayPattern(nested_arr) => {
                1 + array_pattern_nesting_depth(nested_arr)
            }
            internal::Expression::AssignmentPattern(ap) => match ap.left.as_ref() {
                internal::Expression::ObjectPattern(nested_obj) => {
                    1 + pattern_nesting_depth(nested_obj)
                }
                internal::Expression::ArrayPattern(nested_arr) => {
                    1 + array_pattern_nesting_depth(nested_arr)
                }
                _ => 1,
            },
            _ => 1,
        };
        max_depth = max_depth.max(nested_depth);
    }

    max_depth
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
    use tsv_lang::doc;

    match name {
        internal::TSEntityName::Identifier(id) => doc::symbol(id.name.to_u32()),
        internal::TSEntityName::QualifiedName(qn) => doc::concat(vec![
            build_entity_name_doc(&qn.left),
            doc::text("."),
            doc::symbol(qn.right.name.to_u32()),
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
    pub(crate) comments: &'a [internal::Comment],
    /// Extra indent depth for declaration contexts (0 normally, 1+ in multi-declarator)
    /// When > 0, multiline objects/arrays get extra indentation
    /// Uses Cell for interior mutability so doc builders (&self) can set this
    pub(crate) declaration_indent_depth: Cell<usize>,
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
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            interner,
            source,
            comments,
            declaration_indent_depth: Cell::new(0),
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

    /// Check if a pattern expression should expand (print across multiple lines)
    #[allow(clippy::only_used_in_recursion)]
    pub(crate) fn pattern_should_expand(&self, expr: &internal::Expression) -> bool {
        match expr {
            internal::Expression::ObjectPattern(obj) => {
                object_pattern_should_expand(obj, PatternContext::Standalone)
            }
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
            let position = classify_comment(comment, prev_end, curr_start, self.source);

            // Skip trailing comments EXCEPT for first statement (file start)
            if !is_first && matches!(position, CommentPosition::Trailing) {
                continue;
            }

            // Handle inline leading comments (same line as statement)
            if matches!(position, CommentPosition::LeadingInline) {
                self.write_indent();
                self.print_comment(comment);
                self.write(" ");
                printed_any = true;
                last_comment_end = comment.span.end;
                continue;
            }

            // Comment on its own line: check for blank lines
            // Skip blank line preservation at the very start of the program
            // (before the first comment/statement). Prettier removes leading blank lines.
            let is_program_start = is_first && !printed_any;
            if !is_program_start
                && comment.span.start > last_comment_end
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
                    if let Some(skip) = skip_string_or_comment(source, i, end) {
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
            if let Some(skip) = skip_string_or_comment(source, i, end) {
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
            if let Some(skip) = skip_string_or_comment(source, i, end) {
                i = skip;
            }
            i += 1;
        }
        None
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

    /// Build a Doc for leading line comments (comments that start on their own line)
    /// Each line comment gets its own hardline before it.
    /// Returns (comments_doc, has_blank_line) where has_blank_line indicates if there
    /// was a blank line before the comments/content.
    pub(crate) fn build_leading_line_comments_doc(
        &self,
        prev_end: u32,
        curr_start: u32,
    ) -> (Doc, bool) {
        // Bounds check - prev_end can be past curr_start in edge cases
        if prev_end >= curr_start {
            return (doc::concat(vec![]), false);
        }

        let comments: Vec<_> = comments_in_range(self.comments, prev_end, curr_start).collect();
        if comments.is_empty() {
            // Check for blank line even without comments
            let text_between = &self.source[prev_end as usize..curr_start as usize];
            let has_blank = text_between.contains("\n\n")
                || text_between.contains("\n\r\n")
                || text_between.contains("\r\n\r\n");
            return (doc::concat(vec![]), has_blank);
        }

        let mut parts = Vec::new();

        // Check for blank line before first comment
        let first_comment_start = comments[0].span.start;
        let text_before_comments = &self.source[prev_end as usize..first_comment_start as usize];
        let has_blank_line = text_before_comments.contains("\n\n")
            || text_before_comments.contains("\n\r\n")
            || text_before_comments.contains("\r\n\r\n");

        for (i, comment) in comments.iter().enumerate() {
            // Each line comment gets a hardline before it
            // (always add hardline, even for first comment, since we're in an indented block)
            if i > 0 {
                parts.push(doc::hardline());
            }
            parts.push(self.build_comment_doc(comment));
        }

        (doc::concat(parts), has_blank_line)
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
            let between = &self.source[start as usize..comment.span.start_usize()];
            return between.contains('\n');
        }
        false
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
}

// Implement SymbolResolver trait for shared symbol resolution utilities
impl<'a> SymbolResolver for Printer<'a> {
    fn interner(&self) -> &Rc<RefCell<DefaultStringInterner>> {
        &self.interner
    }
}
