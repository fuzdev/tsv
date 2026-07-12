// TypeScript printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct, constructors, and source/comment utilities
// - **analysis.rs**: Pure AST analysis functions (no Printer state needed)
// - **comments.rs**: Comment handling (printing, doc building, filtering)
// - **program.rs**: Program-level printing orchestration (statements, blank lines, comments)
// - **decorators.rs**: Decorator printing (class-level and class-member)
// - **statements/**: Statement printing (declarations, control flow, modules, etc.)
// - **expressions/**: Expression printing (dispatch, literals, functions, patterns, templates,
//   objects, arrays, operators, assignment, conditionals)
// - **types/**: Type annotation printing (TypeScript-specific type syntax)
// - **calls/**: Call and `new` expression formatting (argument wrapping, expand patterns)
// - **chain/**: Member/call chain linearization, grouping, and rendering
// - **class_common.rs**: Shared class-header layout for declaration + expression printers
// - **needs_parens.rs**: Centralized parenthesization logic (`needs_parens(expr, ctx)`)
// - **layout.rs**: Shared hang-indent "break after operator, then indent continuation" doc shapes
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change TypeScript semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod analysis;
mod calls;
mod chain;
mod class_common;
mod comments;
mod decorators;
mod expressions;
mod layout;
mod needs_parens;
mod program;
mod statements;
mod types;

use analysis::needs_isolation_for_hugging;
// Layout predicates re-exported from the crate root for embedders (tsv_svelte's
// {@const} assignment layout reuses Prettier's break-after-operator rules).
pub use analysis::conditional_should_break_after_op;
pub(crate) use analysis::{
    PatternContext, build_entity_name_doc, has_multiline_content, has_newline_before_position,
    is_brace_block_multiline, is_module_path_fluid_call, is_multiline_string_literal,
    is_multiline_template_expression, is_pure_property_chain, is_string_literal,
    object_pattern_should_expand, template_literal_has_newlines,
};
pub(crate) use comments::{
    CommentFilter, CommentSpacing, CommentVec, HeritageKeyword, LeadingGlue,
};
pub use expressions::assignment::should_inline_logical_expression;
pub(crate) use expressions::assignment::{
    arrow_chain_has_return_type, class_expr_has_decorators, is_call_on_member_chain,
    is_curried_arrow_chain, is_curried_arrow_with_return_type, is_literal_member_chain,
    is_poorly_breakable_chain, is_regex_root_chain, is_self_expanding_value,
    is_simple_self_expanding, is_simple_value, is_single_call_on_member_chain,
    is_type_assertion_call,
};
pub(crate) use needs_parens::{ParenContext, is_in_binary, needs_parens};
pub(crate) use types::{should_hug_union_type, unwrap_parenthesized};

use crate::PrinterInputs;
use crate::ast::internal;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use tsv_lang::{
    EmbedContext, OutputBuffer, SharedInterner, Span, SymbolResolver, SymbolToU32, TAB_WIDTH,
    comments_in_range,
    doc::{
        self,
        arena::{DocArena, DocId},
    },
    has_comments_in_range, has_line_comments_in_range, is_format_ignore_directive, printing,
    source_scan::{TriviaProfile, skip_trivia},
};

/// The parent context that routes a curried arrow chain (`(a) => (b) => …`)
/// through a flattened chain layout, mirroring prettier's
/// `printArrowFunctionSignatures` parent-context branches. Set by the enclosing
/// printer (assignment chokepoint, call-argument printer, binary-operand
/// printer) just before the chain's RHS / argument / operand is built; the
/// outermost chain arrow reads and clears it at entry (`replace(None)`) so
/// nested arrows in the chain don't inherit it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ArrowChainContext {
    /// No chain context — arrows use the default break-after-operator path.
    #[default]
    None,
    /// Assignment RHS (`const f = (a) => (b) => …`). The heads join into one
    /// breakable group indented one level after `=` (a leading softline is the
    /// break-after-`=`); all heads share the same indent when they break.
    AssignmentRhs,
    /// Call argument or binaryish operand (`fn((a) => (b) => …)`,
    /// `x ?? ((a) => (b) => …)`) — prettier handles both in one
    /// `printArrowFunctionSignatures` branch. Progressive indent: the first head
    /// stays on the line, the rest indent one level
    /// (`group([sig0, " =>", indent([line, join([" =>", line], rest)])])`).
    CallArgOrBinaryish,
}

/// Printer state for building output
pub struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Embedding context (base indent offset, first-line offset, layout mode, etc.).
    pub(crate) embed: EmbedContext,
    /// Arena allocator for doc nodes (borrowed from caller or locally owned)
    pub(crate) arena: &'a DocArena,
    /// Shared string interner for resolving symbols
    interner: SharedInterner,
    /// Original source code (for extracting raw values, preserving escape sequences, etc.)
    pub(crate) source: &'a str,
    /// Comments from the program (for printing leading/trailing comments)
    pub(crate) comments: &'a [internal::Comment],
    /// Precomputed line break positions for O(log n) line boundary lookups.
    ///
    /// Backs the newline-derived *layout* reads — blank-line preservation
    /// (`has_blank_line_between`) and expansion intent (`has_newline_between`,
    /// and the many free-function `*_fast` call sites that read it directly).
    /// The canonical reprint path ([`crate::format_canonical`]) empties this
    /// table via [`Self::set_canonical`] so those reads collapse (nothing is
    /// "on a new line", no blank lines), erasing authoring intent.
    pub(crate) line_breaks: &'a [u32],
    /// Line breaks used exclusively for *comment* position classification
    /// (`is_same_line`, `classify_comment_fast`, `PartitionedComments`), kept
    /// real even in the canonical path so a comment's trailing/leading/own-line
    /// role stays correct and consecutive line comments never merge onto one
    /// output line. In the normal path this is the same table as `line_breaks`;
    /// they diverge only under [`Self::set_canonical`].
    pub(crate) comment_line_breaks: &'a [u32],
    /// Whether this printer is producing the intent-erased *canonical* reprint
    /// (see [`crate::format_canonical`]). Gates the handful of direct
    /// source-newline scans that don't go through `line_breaks` (type-literal
    /// brace, mapped-type type-argument, own-line decorators).
    pub(crate) canonical: bool,
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
    /// Whether we're inside a curried arrow function with return type.
    /// When true, nested arrows always break after `=>` regardless of their own return type.
    /// Used for: const f = (x: T): H => (y) => expr - ALL arrows break, not just the typed ones.
    pub(crate) in_curried_typed_arrow: Cell<bool>,
    /// Whether to skip curried arrow chain detection (non-id param check).
    /// Set when formatting arrows in call arg expand-last context, matching
    /// prettier's `!args.expandLastArg` in shouldPrintAsChain.
    pub(crate) skip_arrow_chain: Cell<bool>,
    /// Whether to render arrow parameters flat (no break points) — mirrors
    /// prettier's `expandLastArg`/`expandFirstArg` path, which prints the
    /// signature with `removeLines` so an expanded last-arg arrow keeps its
    /// params on one line and only the body breaks. Without this, a force-broken
    /// arrow could shatter a destructuring param (`([a, b])`) instead of falling
    /// through to the all-args-broken-out layout. Set around the arrow-doc build
    /// in the expand-last-arg call-argument states.
    pub(crate) expand_last_arg_flat_params: Cell<bool>,
    /// Span of the ObjectExpression at the leftmost position of an arrow body that must
    /// be wrapped in parens to avoid block ambiguity — `() => ({}) as Logger`,
    /// `() => ({}).prop`, `() => ({}) && a`, `() => ({}).b++`. Matches prettier's
    /// `startsWithNoLookaheadToken` traversal. Keyed by span (not consumed) so a chain
    /// rebuilding its base across conditional-group variants wraps consistently, and a
    /// same-shaped object nested deeper (a call argument) never matches.
    pub(crate) arrow_body_object_parens_target: Cell<Option<Span>>,
    /// Span of the object/function/class node that starts an expression statement
    /// and must be wrapped in parens, even when nested as the leftmost token of a
    /// member/binary/etc. chain: `(class {}).foo`, `({}).foo`, `(class {}) + 1`,
    /// `({a: 1}).b().c()`. Matches prettier's `startsWithNoLookaheadToken` traversal.
    /// Keyed by span (not consumed, like `arrow_body_object_parens_target`) so a chain
    /// rebuilding its base across conditional-group variants wraps consistently; cleared
    /// once per statement in `build_expression_statement`.
    pub(crate) expr_stmt_paren_target: Cell<Option<Span>>,
    /// The parent context for a curried arrow-chain value, set by the enclosing
    /// printer (assignment chokepoint, call-argument printer, binary-operand
    /// printer) just before the chain is built. The arrow printer reads and
    /// clears it at entry so the outermost chain arrow picks the right flattened
    /// layout (assignment-RHS vs progressive call-arg/binaryish) while nested
    /// arrows don't inherit it. Mirrors prettier routing the parent context
    /// (`args.assignmentLayout`, `isCallLikeExpression(parent)`,
    /// `isBinaryish(parent)`) into `printArrowFunctionSignatures`.
    pub(crate) arrow_chain_context: Cell<ArrowChainContext>,
    /// Whether we're building the **init** clause of a C-style `for` header.
    /// In that clause an `in` binary expression must be parenthesized to keep it
    /// distinct from the `for (x in y)` separator — prettier parenthesizes every
    /// `in` anywhere lexically under the init (not just where strictly required),
    /// so this flag is set while building the init subtree and propagates through
    /// it (including nested function/class bodies). Read by `needs_parens` and the
    /// surgical `in`-wrap at positions that build an expression without a
    /// `needs_parens` check (assignment RHS, ternary branches/test).
    /// Uses Cell for interior mutability so doc builders (&self) can set this.
    pub(crate) in_for_init: Cell<bool>,
    /// Scoped argument-doc share map for member-chain building: an argument
    /// expression's pointer → its already-built [`Self::build_arg_expression_doc`]
    /// `DocId`. A member chain renders the same group **flat** (`print_group`) and
    /// **expanded** (`print_group_expanded`) across `conditional_group` candidates;
    /// without sharing, the recursive arg build runs once per candidate and a nested
    /// chain in a call arg compounds to O(4^depth) — the member-chain rebuild blowup.
    /// The flat and expanded builds differ in
    /// Printer state **only** via `skip_arrow_chain` / `expand_last_arg_flat_params`
    /// (the sole `.set` sites in `calls/chain_args.rs`); every other flag is
    /// statement-constant during a chain or set identically by the shared AST
    /// traversal, so a given node is reached under identical state in both candidates.
    /// Hence the cache is consulted only when [`Self::chain_arg_share_eligible`]
    /// (active + both of those flags clear), making a hit byte-identical to a rebuild.
    /// Active only between `enter_chain_arg_share`/`exit_chain_arg_share` (the outermost
    /// `build_chain_doc`); keyed by node pointer (stable — the AST arena is immutable
    /// during formatting).
    pub(crate) chain_arg_share: RefCell<HashMap<usize, DocId>>,
    pub(crate) chain_arg_share_active: Cell<bool>,
    /// Expand-last-arg body reuse: `(body-expr span start, pre-built body DocId)`.
    /// The call/new expand-last paths build an arrow's call body **once** up front
    /// and set this before building the whole-arrow argument doc via
    /// `build_args_split_last`; [`Self::build_arrow_body_doc`] then returns the
    /// pre-built DocId for that exact node instead of rebuilding it. Building the
    /// body twice — inside the whole arrow *and* separately for the break-body
    /// state — recurses into itself for a call-bodied arrow whose body is another
    /// such call (`f(lead, x => f(lead, y => …))`), making the doc-node count
    /// O(2^depth). Reusing the one build keeps it linear and is byte-identical (the
    /// injected DocId is exactly what the arrow's own body build would produce for a
    /// call body — `build_arrow_body_doc` returns `build_expression_doc` there).
    /// Keyed by span (unique per source position); nested expand-last calls
    /// save/restore, so only the node currently being reused ever matches.
    pub(crate) arrow_body_inject: Cell<Option<(u32, DocId)>>,
}

impl<'a> Printer<'a> {
    /// Create a new printer borrowing the given arena, [`PrinterInputs`], and
    /// embedding context.
    ///
    /// `buffer_capacity` pre-sizes the output buffer: the source length for the
    /// rendering path, or `0` for doc-only embedding builds that never write
    /// output (see `make_printer` / `make_doc_printer` in `lib.rs`).
    pub(crate) fn with_context(
        arena: &'a DocArena,
        inputs: &PrinterInputs<'a>,
        embed: EmbedContext,
        buffer_capacity: usize,
    ) -> Self {
        Self {
            buffer: OutputBuffer::with_capacity(buffer_capacity),
            indent_level: 0,
            embed,
            arena,
            interner: Rc::clone(&inputs.interner),
            source: inputs.source,
            comments: inputs.comments,
            line_breaks: inputs.line_breaks,
            // Normal path: comment classification shares the one real table. The
            // canonical path re-points `line_breaks` at an empty table but leaves
            // this one real (see `set_canonical`).
            comment_line_breaks: inputs.line_breaks,
            canonical: false,
            declaration_indent_depth: Cell::new(0),
            is_expression_statement: Cell::new(false),
            in_top_level_assignment: Cell::new(false),
            in_curried_typed_arrow: Cell::new(false),
            skip_arrow_chain: Cell::new(false),
            expand_last_arg_flat_params: Cell::new(false),
            arrow_body_object_parens_target: Cell::new(None),
            expr_stmt_paren_target: Cell::new(None),
            arrow_chain_context: Cell::new(ArrowChainContext::None),
            in_for_init: Cell::new(false),
            chain_arg_share: RefCell::new(HashMap::new()),
            chain_arg_share_active: Cell::new(false),
            arrow_body_inject: Cell::new(None),
        }
    }

    /// Arm expand-last-arg body reuse for the node at `span`: the next
    /// [`Self::build_arrow_body_doc`] for that exact node returns `doc` instead of
    /// rebuilding it. Returns the previous injection (nested expand-last calls each
    /// arm their own, restoring the outer one after). See the `arrow_body_inject` field.
    pub(crate) fn inject_arrow_body(&self, span: u32, doc: DocId) -> Option<(u32, DocId)> {
        self.arrow_body_inject.replace(Some((span, doc)))
    }

    /// Restore the previous expand-last-arg body injection (from `inject_arrow_body`).
    pub(crate) fn restore_arrow_body_inject(&self, prev: Option<(u32, DocId)>) {
        self.arrow_body_inject.set(prev);
    }

    /// Wrap `doc` in parens when `expr` is an `in` binary built directly inside a
    /// `for` header init. Used at positions that build an expression *without* a
    /// [`needs_parens`] check — assignment RHS, ternary branches/test, and the
    /// init clause's own expression/declarator — so the for-init `in` rule still
    /// applies there. Positions that already route through [`needs_parens`] (call
    /// args, object values, binary operands, …) get the same wrap via that path.
    #[inline]
    pub(crate) fn wrap_for_init_in(&self, expr: &internal::Expression<'_>, doc: DocId) -> DocId {
        if self.in_for_init.get() && is_in_binary(expr) {
            self.arena.parens(doc)
        } else {
            doc
        }
    }

    /// Print-context-aware wrapper over the free [`needs_parens`]: supplies the
    /// ambient for-header-init flag so the for-init `in` rule applies at every
    /// call site without threading the flag by hand. Prefer this inside `Printer`
    /// methods; the free function (which still requires the flag explicitly) is
    /// only for the few free helpers that have no `self`.
    #[inline]
    pub(crate) fn needs_parens(&self, expr: &internal::Expression<'_>, ctx: ParenContext) -> bool {
        needs_parens(expr, ctx, self.in_for_init.get())
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
        let context_offset = if self.embed.first_line_offset > 0 {
            if self.current_column() == 0 {
                self.embed.first_line_offset
            } else {
                0
            }
        } else {
            self.embed.base_indent_offset * TAB_WIDTH
        };
        let current_col = self.current_column() + context_offset;
        // Render into the arena-parked scratch: one warm buffer across the
        // file's renders (the whole program standalone; one per template
        // expression when Svelte-embedded) instead of an alloc/free per call.
        let mut output = self.arena.take_render_scratch();
        {
            let interner = self.interner.borrow();
            // Source-aware resolver so `DocText::SourceSpan` nodes (verbatim
            // comment/literal slices) resolve without a `DocArena` lifetime.
            let resolver = doc::SourceTextResolver {
                inner: &*interner,
                source: self.source,
            };
            doc::arena_print_doc_with_indent_resolved_into(
                self.arena,
                d,
                &self.embed,
                current_col,
                self.indent_level,
                &resolver,
                &mut output,
            );
        }
        self.write(&output);
        self.arena.park_render_scratch(output);
    }

    /// Render an arena DocId to a flat string with effectively infinite width.
    pub(crate) fn render_arena_doc_flat(&self, d: DocId) -> String {
        let interner = self.interner.borrow();
        let resolver = doc::SourceTextResolver {
            inner: &*interner,
            source: self.source,
        };
        doc::arena_print_doc_flat_resolved(self.arena, d, &self.embed, &resolver)
    }

    /// Get the formatted output
    pub(crate) fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Set the indent level (for formatting expressions in nested contexts)
    pub(crate) fn set_indent_level(&mut self, level: usize) {
        self.indent_level = level;
    }

    /// Get the current column position (for doc-builder width calculations)
    pub(crate) fn current_column(&self) -> usize {
        self.buffer.current_column(TAB_WIDTH)
    }

    /// Compute the visual indent width at a source position.
    ///
    /// Finds the start of the line containing `pos` and measures the leading
    /// whitespace visual width (tabs count as `tab_width` chars).
    pub(crate) fn source_indent_visual(&self, pos: u32) -> usize {
        let pos = pos as usize;
        let line_start = self.source[..pos].rfind('\n').map_or(0, |i| i + 1);
        printing::visual_width(&self.source[line_start..pos], TAB_WIDTH)
    }

    /// Check if two positions are on the same line (O(log n) binary search).
    ///
    /// Reads `comment_line_breaks` (not `line_breaks`) so comment position
    /// classification stays correct even when the canonical path has emptied the
    /// layout table. In the normal path the two tables are identical.
    #[inline]
    pub(crate) fn is_same_line(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::is_same_line_fast(self.comment_line_breaks, prev_end, curr_start)
    }

    /// Switch this printer into the intent-erased *canonical* reprint mode.
    ///
    /// Empties the layout line-break table (so `has_blank_line_between` /
    /// `has_newline_between` and every direct `*_fast` reader collapse — no blank
    /// lines, nothing forced multiline by a source newline) and sets the
    /// `canonical` flag that gates the direct source-newline scans. Comment
    /// classification keeps the real `comment_line_breaks` table, so comments are
    /// preserved losslessly (merely re-placed deterministically). Cold path:
    /// called once, right after construction, before any doc is built.
    pub(crate) fn set_canonical(&mut self) {
        self.canonical = true;
        // `&[]` is `&'static`, which coerces to the field's `'a`.
        self.line_breaks = &[];
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

    /// Build expression doc with IsolatedGroup wrapping for huggable expressions.
    ///
    /// Wraps templates and arrow-with-template-body in `isolated_group` to prevent
    /// internal breaks from forcing parent calls/arrays to break (enables hugging).
    pub(crate) fn build_huggable_expression_doc(&self, expr: &internal::Expression<'_>) -> DocId {
        let d = self.d();
        let base_doc = self.build_arg_expression_doc(expr);
        if needs_isolation_for_hugging(expr) {
            d.isolated_group(base_doc)
        } else {
            base_doc
        }
    }

    /// Whether `build_arg_expression_doc` may share its result via `chain_arg_share`.
    /// True only while a member chain is building (`chain_arg_share_active`) AND the two
    /// flags that make the flat vs expanded chain-group builds diverge are clear — see
    /// the `chain_arg_share` field doc. When either is set we're building an arrow arg in
    /// the expand-last / curried path, which the expanded candidate builds under different
    /// state, so it must not share.
    pub(crate) fn chain_arg_share_eligible(&self) -> bool {
        self.chain_arg_share_active.get()
            && !self.skip_arrow_chain.get()
            && !self.expand_last_arg_flat_params.get()
    }

    /// Activate `chain_arg_share` for the outermost `build_chain_doc` only. Returns the
    /// prior active state; nested chains observe `true` and become no-ops (the map
    /// persists across the whole top-level chain so every nesting level shares).
    pub(crate) fn enter_chain_arg_share(&self) -> bool {
        let was_active = self.chain_arg_share_active.get();
        if !was_active {
            self.chain_arg_share_active.set(true);
            self.chain_arg_share.borrow_mut().clear();
        }
        was_active
    }

    /// Deactivate + clear `chain_arg_share` when leaving the outermost `build_chain_doc`
    /// (`was_active` false). Nested exits are no-ops.
    pub(crate) fn exit_chain_arg_share(&self, was_active: bool) {
        if !was_active {
            self.chain_arg_share_active.set(false);
            self.chain_arg_share.borrow_mut().clear();
        }
    }

    /// Check if identifier has a complex type annotation (nested generics)
    ///
    /// Corresponds to prettier's `hasComplexTypeAnnotation`:
    /// - Type reference with >1 type parameters
    /// - At least one type param has nested generics OR is a conditional type
    ///
    /// Example: `Map<string, Array<number>>` - Map has 2 params, second has nested generic
    pub(crate) fn id_has_complex_type_annotation(&self, expr: &internal::Expression<'_>) -> bool {
        let type_ann = match expr {
            internal::Expression::Identifier(id) => id.type_annotation(),
            internal::Expression::ObjectPattern(obj) => obj.type_annotation.as_ref(),
            internal::Expression::ArrayPattern(arr) => arr.type_annotation.as_ref(),
            _ => None,
        };

        type_ann.is_some_and(|ann| self.type_has_complex_annotation(ann.type_annotation))
    }

    /// Check if a type has complex nested type parameters
    fn type_has_complex_annotation(&self, ts_type: &internal::TSType<'_>) -> bool {
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
    fn type_has_nested_generics(&self, ts_type: &internal::TSType<'_>) -> bool {
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
        type_params: Option<&internal::TSTypeParameterDeclaration<'_>>,
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
    pub(crate) fn id_has_complex_destructuring(&self, expr: &internal::Expression<'_>) -> bool {
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
            if let Some(new_i) = tsv_lang::source_scan::skip_comment(bytes, i, end_pos) {
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

    /// Position to start scanning from when looking for comments that trail the
    /// last argument (between the argument and the closing paren).
    ///
    /// For a spread element whose stripped grouping parens hide comments
    /// (`...( /* c */ x )`), the spread span extends past the inner argument, so
    /// scan from the inner argument's end to find those comments; otherwise scan
    /// from the argument's own end.
    pub(crate) fn last_arg_comment_scan_start(&self, arg: &internal::Expression<'_>) -> u32 {
        if let internal::Expression::SpreadElement(spread) = arg
            && self.has_comments_between(spread.argument.span().end, spread.span.end)
        {
            spread.argument.span().end
        } else {
            arg.span().end
        }
    }

    /// Find the first occurrence of a byte in source between `start` and `end`
    /// that is NOT inside a comment. Returns absolute position.
    pub(crate) fn find_char_outside_comments(&self, start: u32, end: u32, ch: u8) -> Option<u32> {
        tsv_lang::source_scan::find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            ch,
        )
        .map(|pos| pos as u32)
    }

    /// Position of the comma separating two consecutive list items in
    /// `[prev_end, next_start)`, ignoring commas inside comments. The shared anchor
    /// for splitting a gap's comments into before-comma (trailing the previous item)
    /// and after-comma (leading the next / stranded). Falls back to `next_start` when
    /// none is found — a defensive case (list items always have a real separator);
    /// the fallback keeps the split lossless: the whole gap then reads as before-comma
    /// (trailing the previous item), so no comment is dropped.
    pub(crate) fn comma_between(&self, prev_end: u32, next_start: u32) -> u32 {
        self.find_char_outside_comments(prev_end, next_start, b',')
            .unwrap_or(next_start)
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

    /// Whether comments in the range force the following value onto its own line.
    /// Two comment shapes hang the value: a **line** comment (runs to
    /// end-of-line — inlining would swallow the value), and a **multiline** block
    /// comment the author wrote on its own line (`kw⏎/* … */⏎v`, i.e. a newline
    /// after it). Everything else collapses to the inline form (`kw /* c */ v`):
    /// a single-line block in *any* position (glued, trailing the keyword, or
    /// own-line), and a **glued** multiline block — one whose operand shares the
    /// comment's closing line (`kw /* …⏎… */ v`), the way prettier keeps it.
    ///
    /// This is the gate for the keyword→value gaps (as/satisfies,
    /// heritage/conditional `extends`, keyof/typeof/readonly, infer,
    /// type-param constraint/default, predicate `is`, indexed access) and the
    /// type-alias `=` layout. Keying the multiline case on the newline *after*
    /// the comment (not before) keeps it idempotent: a block glued to the value
    /// stays inline even at line start in already-broken output, and only an
    /// authored break hangs it. Contrast
    /// [`Self::comment_forces_following_own_line`], which *also* hangs a
    /// single-line own-line block — the two differ only in that `c.multiline`
    /// guard; use that variant only at the two carve-out sites where prettier
    /// *keeps* that break (binary/logical operands, `export default`).
    pub(crate) fn comments_force_own_line_between(&self, start: u32, end: u32) -> bool {
        self.any_comment_with_next(start, end, |c, next| {
            !c.is_block || (c.multiline && self.has_newline_between(c.span.end, next))
        })
    }

    /// Whether a comment in `(start, end)` forces the *following* value onto its own
    /// line: a line comment (runs to EOL), or a block comment with a newline AFTER it
    /// — toward the next comment, or `end` for the last (prettier's
    /// `hasLeadingOwnLineComment`). Keying on the newline *after* the comment (not
    /// before) keeps the layout idempotent: a block glued to the value (`/* c */ v`,
    /// even at line start in already-broken output) stays inline, only an authored
    /// break (`/* c */⏎v`) forces the value down. Used only at the two carve-out
    /// sites where prettier *keeps* the operand break — binary/logical operands
    /// (`operators.rs`) and `export default` (`modules/mod.rs`) — so hanging is the
    /// smaller (indent-only) divergence than collapsing. Contrast
    /// [`Self::comments_force_own_line_between`], which collapses an authored
    /// own-line single-line block inline; that is the gate for every other
    /// keyword→value gap.
    pub(crate) fn comment_forces_following_own_line(&self, start: u32, end: u32) -> bool {
        self.any_comment_with_next(start, end, |c, next| {
            !c.is_block || self.has_newline_between(c.span.end, next)
        })
    }

    /// Whether a comment in `(start, end)` is separated from what follows it (the
    /// next comment, or `end`) by a blank line. Used where a blank line after a
    /// comment is itself a break trigger — e.g. a ternary branch (`a ? /* c */⏎⏎b`),
    /// where prettier breaks on the blank even though the own-line comment alone
    /// does not.
    pub(crate) fn comment_followed_by_blank(&self, start: u32, end: u32) -> bool {
        self.any_comment_with_next(start, end, |c, next| {
            self.has_blank_line_between(c.span.end, next)
        })
    }

    /// Scan the comments in `(start, end)` with one-ahead lookahead, returning true
    /// if `pred(comment, next_start)` holds for any — where `next_start` is the
    /// following comment's start, or `end` for the last. The shared primitive behind
    /// the gap predicates above (each keys a per-comment rule on the gap to whatever
    /// follows it). `peekable` over `comments_in_range`, so no allocation.
    fn any_comment_with_next(
        &self,
        start: u32,
        end: u32,
        pred: impl Fn(&internal::Comment, u32) -> bool,
    ) -> bool {
        let mut comments = comments_in_range(self.comments, start, end).peekable();
        while let Some(c) = comments.next() {
            let next = comments.peek().map_or(end, |n| n.span.start);
            if pred(c, next) {
                return true;
            }
        }
        false
    }

    /// Whether a single comment occupies its own physical line — a line comment
    /// (always runs to end-of-line), or a block comment with only horizontal
    /// whitespace then a newline before it (`…⏎/* c */…`). The precise "starts a
    /// fresh line" test: `has_newline_before_position` walks back over spaces/tabs
    /// from the comment, so a block following another comment on the same line
    /// (`/* a */ /* b */`) is *not* own-line. Unlike the neighbor-anchored
    /// `is_same_line(prev, …)` / `has_newline_between(prev, …)` checks, it takes no
    /// anchor — each comment is judged against whatever immediately precedes it.
    pub(crate) fn is_own_line_comment(&self, comment: &internal::Comment) -> bool {
        !comment.is_block || has_newline_before_position(self.source, comment.span.start)
    }

    /// Whether a comment must occupy its own line rather than gluing inline to the
    /// operator that precedes it: a line comment, a multiline block, or an own-line
    /// block (a newline precedes it). This is exactly the negation of the single-line
    /// glued block that
    /// [`build_rhs_comments_glued_opt`](Self::build_rhs_comments_glued_opt) hugs across
    /// a source newline, so the two stay in lockstep.
    pub(crate) fn comment_forces_own_line(&self, comment: &internal::Comment) -> bool {
        comment.multiline || self.is_own_line_comment(comment)
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
        F: Fn(&T) -> Span,
    {
        let between = items.windows(2).any(|pair| {
            self.has_line_comments_between(get_span(&pair[0]).end, get_span(&pair[1]).start)
        });
        let trailing = items
            .last()
            .is_some_and(|last| self.has_line_comments_between(get_span(last).end, end_boundary));
        between || trailing
    }

    /// Check if a bracket-delimited list contains own-line single-line block comments.
    ///
    /// Generic version for tuples, array patterns, and other `[...]`-delimited lists.
    /// A block comment is "own-line" when it's not on the same line as either the
    /// preceding element (or opening bracket) or the following element (or closing bracket).
    pub(crate) fn has_own_line_block_comments_in_bracket_list<T, F>(
        &self,
        span: Span,
        items: &[T],
        get_span: F,
    ) -> bool
    where
        F: Fn(&T) -> Span,
    {
        let open_bracket = span.start;

        for comment in comments_in_range(self.comments, open_bracket + 1, span.end - 1) {
            if !comment.is_block || comment.multiline {
                continue;
            }

            // Skip comments that are inside an element (they belong to that element, not this list)
            let inside_element = items.iter().any(|e| {
                let s = get_span(e);
                comment.span.start >= s.start && comment.span.end <= s.end
            });
            if inside_element {
                continue;
            }

            // Find preceding element end (or opening bracket)
            let prev_boundary = items
                .iter()
                .map(|e| get_span(e).end)
                .take_while(|&end| end <= comment.span.start)
                .last()
                .unwrap_or(open_bracket);

            // Skip if on same line as preceding element (trailing inline comment)
            if self.is_same_line(prev_boundary, comment.span.start) {
                continue;
            }

            // Find the following element start, if any.
            let next_elem_start = items
                .iter()
                .map(|e| get_span(e).start)
                .find(|&start| start > comment.span.end);

            match next_elem_start {
                // A comment between two elements is own-line when it doesn't share
                // a line with the following element.
                Some(next) => {
                    if !self.is_same_line(comment.span.end, next) {
                        return true;
                    }
                }
                // A trailing comment before the closing bracket is a dangling
                // comment: own-line whenever it cleared the same-line-as-preceding
                // check above, even if the closing bracket was pulled onto its line
                // (`/* c */ ]`). Prettier expands in that case.
                None => return true,
            }
        }
        false
    }

    /// Find the closing `)` between a start position and end boundary.
    ///
    /// Scans the source to find the `)` that closes the params. Returns
    /// the position AFTER the `)` for use as a boundary.
    pub(crate) fn find_closing_paren(&self, start: u32, end: u32) -> Option<u32> {
        let source = self.source.as_bytes();
        let end = (end as usize).min(source.len());
        let mut depth = 0;
        let mut i = start as usize;

        while i < end {
            if let Some(past) = skip_trivia(source, i, end, TriviaProfile::JS) {
                i = past;
                continue;
            }
            match source[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i + 1) as u32);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Position of the `)` that closes the `(` at `open` — the index OF the `)`,
    /// located with the depth-tracked, comment-aware scan over the rest of the
    /// source. Use when only the open-paren position is known and the close lies
    /// somewhere ahead; call `find_closing_paren` directly when a tighter search
    /// bound is available. Returns `None` if no matching `)` is found.
    pub(crate) fn matching_close_paren(&self, open: u32) -> Option<u32> {
        self.find_closing_paren(open, self.source.len() as u32)
            .map(|after| after - 1)
    }

    /// Find the end position of a keyword in source text.
    ///
    /// Searches backward from `end` for `keyword` as a whole word (not part of
    /// an identifier). Returns the byte position after the last character of the keyword,
    /// or `None` if not found.
    pub(crate) fn find_keyword_end(&self, keyword: &str, start: u32, end: u32) -> Option<u32> {
        // The LAST whole-word occurrence that is not inside a comment — so a
        // keyword buried in a comment (`from /* from */ 'x'`) isn't mistaken for
        // the real one (which dropped/relocated the comment), while a later real
        // keyword still wins over an earlier identifier containing it.
        tsv_lang::source_scan::rfind_keyword(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            keyword.as_bytes(),
            TriviaProfile::JS,
        )
        .map(|i| (i + keyword.len()) as u32)
    }

    /// Find the `=>` token position for an arrow function.
    ///
    /// Computes the signature end from the arrow's structure and scans for `=>`.
    /// Returns the position of `=` in `=>`, or the body start as fallback.
    pub(crate) fn find_arrow_token_for(
        &self,
        arrow: &internal::ArrowFunctionExpression<'_>,
    ) -> u32 {
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
        let end = (end as usize).min(source.len());
        let mut i = start as usize;

        while i + 1 < end {
            if let Some(past) = skip_trivia(source, i, end, TriviaProfile::JS) {
                i = past;
                continue;
            }
            if source[i] == b'=' && source[i + 1] == b'>' {
                return Some(i as u32);
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
        let end = (end as usize).min(source.len());
        tsv_lang::source_scan::find_keyword(
            source,
            start as usize,
            end,
            keyword.as_bytes(),
            TriviaProfile::JS,
        )
        .map(|i| i as u32)
    }

    /// Find the position of the first non-whitespace, non-comment token after `start`.
    ///
    /// Skips spaces, tabs, newlines, line comments (`//`), and block comments (`/* */`).
    /// Used to find where the first modifier keyword or identifier begins after decorators.
    pub(crate) fn find_first_token_after(&self, start: u32) -> u32 {
        let bytes = self.source.as_bytes();
        let mut pos = start as usize;
        while pos < bytes.len() {
            match bytes[pos] {
                b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
                b'/' if bytes.get(pos + 1) == Some(&b'/') => {
                    // Skip line comment
                    pos += 2;
                    while pos < bytes.len() && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                }
                b'/' if bytes.get(pos + 1) == Some(&b'*') => {
                    // Skip block comment
                    pos += 2;
                    while pos + 1 < bytes.len() {
                        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                            pos += 2;
                            break;
                        }
                        pos += 1;
                    }
                }
                _ => return pos as u32,
            }
        }
        pos as u32
    }

    /// Find the source position of a keyword that follows decorators.
    ///
    /// Searches for `keyword` in the source text after the last decorator's end.
    /// Returns `fallback` if there are no decorators or the keyword isn't found.
    pub(crate) fn find_keyword_after_decorators(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        keyword: &str,
        fallback: u32,
    ) -> u32 {
        decorators
            .and_then(|decs| decs.last())
            .and_then(|last| {
                // Comment-aware + word-boundaried, so a keyword inside a comment
                // between the decorator and the declaration (`@dec /* class */
                // class C {}`) isn't matched (which would drop the comment).
                self.find_keyword_in_range(last.span.end, self.source.len() as u32, keyword)
            })
            .unwrap_or(fallback)
    }

    /// Check if any comment in the range is a format-ignore directive.
    /// Used to emit the next node as raw source text instead of formatting.
    fn has_format_ignore_in_range(&self, start: u32, end: u32) -> bool {
        comments_in_range(self.comments, start, end)
            .any(|c| is_format_ignore_directive(c.content(self.source)))
    }

    /// Emit a node's source span verbatim. Used to round-trip the source of a
    /// format-ignored node (statement, block statement, object/pattern
    /// property, class/enum/interface/type-literal member) instead of
    /// reformatting it.
    /// Trailing whitespace is trimmed: a node's significant tokens never end in
    /// whitespace, and prettier never preserves it — some spans (e.g. a
    /// `TSConstructSignatureDeclaration`'s) over-extend to the next line's start.
    fn raw_source_doc(&self, span: Span) -> DocId {
        self.raw_source_range(span.start, span.end)
    }

    /// Emit `[start, end)` of the source verbatim. Like `raw_source_doc` but for a
    /// format-ignored member whose verbatim slice must exclude a separator
    /// the surrounding loop emits itself (e.g. a type-literal member's `;`), so
    /// the terminator isn't duplicated.
    ///
    /// Emitted as a `source_span` over the whitespace-trimmed sub-span — an
    /// ignored region can be large, and the verbatim slice needs no pool copy.
    fn raw_source_range(&self, start: u32, end: u32) -> DocId {
        let trimmed = self.source[start as usize..end as usize].trim_end();
        let span = Span {
            start,
            end: start + trimmed.len() as u32,
        };
        self.d().source_span(span, self.source)
    }

    /// Emit an identifier-name doc node — the doc-side name-emission seam.
    /// Span-identity names render as verbatim source (`DocText::SourceSpan`
    /// with deferred width — identifier names are newline-free, and the lazy
    /// measure matches `Symbol`'s zero build-time cost); escaped names defer
    /// to the interner (`DocText::Symbol`), resolved at render exactly as
    /// before.
    pub(in crate::printer) fn ident_name_doc(
        &self,
        name: internal::IdentName,
        name_start: u32,
    ) -> DocId {
        let d = self.d();
        match name.escaped {
            Some(sym) => d.symbol(sym.to_u32()),
            None => d.source_span_ident(Span::new(name_start, name_start + name.raw_len as u32)),
        }
    }

    /// [`Self::ident_name_doc`] for an `Identifier` node (the name is the
    /// leading token of the node span).
    pub(in crate::printer) fn identifier_name_doc(&self, id: &internal::Identifier<'_>) -> DocId {
        self.ident_name_doc(id.ident_name(), id.span.start)
    }

    /// Run `f` over a name channel resolved at `name_start` — the compare/width
    /// seam. Span-identity names borrow the source slice; escaped names resolve
    /// the interned decoded form (so an escaped name still compares decoded).
    pub(in crate::printer) fn with_ident_name_at<R>(
        &self,
        name: internal::IdentName,
        name_start: u32,
        f: impl FnOnce(&str) -> R,
    ) -> R {
        match name.escaped {
            Some(sym) => self.with_resolved_symbol(sym, f),
            None => {
                f(&self.source[name_start as usize..name_start as usize + name.raw_len as usize])
            }
        }
    }

    /// [`Self::with_ident_name_at`] for an `Identifier` node.
    pub(in crate::printer) fn with_ident_name<R>(
        &self,
        id: &internal::Identifier<'_>,
        f: impl FnOnce(&str) -> R,
    ) -> R {
        self.with_ident_name_at(id.ident_name(), id.span.start, f)
    }
}

// Implement SymbolResolver trait for shared symbol resolution utilities
impl<'a> SymbolResolver for Printer<'a> {
    fn interner(&self) -> &SharedInterner {
        &self.interner
    }
}
