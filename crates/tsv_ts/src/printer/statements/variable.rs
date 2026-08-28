// Variable declaration printing for TypeScript

use super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::layout::{fluid_after_operator, hang_after_operator};
use crate::printer::{
    CommentFilter, CommentSpacing, CommentVec, ContinuationValue, LeadingGlue, OwnedCommentEffect,
    ParenContext, analysis, class_expr_has_decorators, conditional_should_break_after_op,
    is_call_on_member_chain, is_curried_arrow_chain, is_curried_arrow_chain_that_breaks,
    is_literal_member_chain, is_module_path_fluid_call, is_multiline_string_literal,
    is_poorly_breakable_chain, is_pure_property_chain, is_regex_root_chain,
    is_self_expanding_value, is_simple_self_expanding, is_simple_value,
    is_single_call_on_member_chain, is_string_literal, is_type_assertion_call, needs_parens,
    should_inline_logical_expression,
};
use smallvec::{SmallVec, smallvec};
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::doc::{DocBuf, GroupId};
use tsv_lang::{INDENT, PRINT_WIDTH, Span};

/// Build the fluid assignment layout: break after `=` only when the full line
/// exceeds print_width. Uses indentIfBreak so the RHS is evaluated independently.
/// Matches Prettier's assignment.js lines 59-67.
///
/// Wrapped in its own group so the marker's fits() evaluation doesn't see
/// trailing elements like ";" that would cause incorrect breaking.
fn build_fluid_assignment_doc(d: &DocArena, id_doc: DocId, init_doc: DocId) -> DocId {
    d.group(d.concat(&[
        id_doc,
        d.text(" ="),
        fluid_after_operator(d, init_doc, GroupId::Assignment),
    ]))
}

/// One declarator's `=` and what sits around it — the spans
/// [`Printer::build_declarator_init_doc`] bounds its comment reads with, plus the two
/// answers its caller already resolved. Grouped because they are one fact about one gap,
/// and because the layout arms read them in pairs.
pub(in crate::printer) struct DeclaratorEqGap {
    /// End of the binding, past a definite `!` — the near edge of the `=` gap.
    pub id_end: u32,
    /// Position of the `=`.
    pub equals_pos: u32,
    /// Start of the initializer — the far edge of the `=` gap.
    pub init_start: u32,
    /// Whether the binding→`=` gap holds a comment to emit. Only BLOCK comments reach
    /// here; a line comment there took its caller's continuation path.
    pub has_comments_before_eq: bool,
    /// Whether the `=`→initializer gap holds a comment to emit.
    pub has_comments_after_eq: bool,
}

/// What [`Printer::build_declarator_init_doc`] needs from the position a declarator sits
/// in. Every field is a fact the caller already has in hand: the binding's printed doc, the
/// spans bounding its `=`, and what the gaps around that `=` hold.
pub(in crate::printer) struct DeclaratorInitInputs<'e, 'a> {
    /// The declarator itself — read for its binding (the break-lhs arms rebuild the id doc
    /// with a wrapping / non-wrapping type annotation) and its definite `!`.
    pub declarator: &'e internal::VariableDeclarator<'a>,
    /// Its initializer. Passed rather than read back off `declarator.init`, since the caller
    /// has already matched on it to get here.
    pub init: &'e Expression<'a>,
    /// Start of the enclosing DECLARATION — the anchor for the expandable-member-call
    /// width check's source indent, which measures the call head from the statement's
    /// own column.
    pub decl_start: u32,
    /// The printed binding.
    pub id_doc: DocId,
    /// `can_break(id_doc)` — prettier's `canBreak(leftDoc)`, which decides the fluid arm.
    pub can_break_left: bool,
    /// The `=` and what the gaps on either side of it hold.
    pub gap: DeclaratorEqGap,
    /// Whether the declaration hardline-separates its declarators (a multi-declarator
    /// statement with initializers). Always `false` under a for header, which separates
    /// on width.
    pub should_break: bool,
    /// Whether this is the declaration's FIRST declarator — the break-lhs arms apply to it
    /// alone unless the list is already broken.
    pub is_first: bool,
}

impl<'a> Printer<'a> {
    /// The `<id> = <value>` half of ONE variable declarator — prettier's `printAssignment`
    /// for a `VariableDeclarator`, stated once for the two positions a declarator occurs
    /// in: a declaration statement ([`Printer::build_variable_declaration_doc`]) and a
    /// `for` header's init clause (`build_for_init_doc`). Prettier prints both through
    /// `printVariableDeclaration` and differs only in the separator BETWEEN declarators
    /// (`hardline` in a statement, `line` under a for header), so the layout inside one
    /// declarator cannot be allowed to differ either — the header spelling its own flat
    /// `" = " + value` is how that position came to have no break-after-operator arm at
    /// all, and no over-width initializer written there could be a fixture input.
    ///
    /// The caller supplies the id doc, the comment answers it resolved around its own `=`,
    /// and `value` — the position's own value builder, which is the one thing that really
    /// does differ (a header declarator's initializer carries the `[~In]` paren shell its
    /// clause needs, and each position resolves its own value-head freeze).
    ///
    /// `value` is `&dyn` rather than a generic: the body is long and its two callers would
    /// otherwise monomorphize the whole cascade twice for no gain — every layout arm calls
    /// it at most once.
    ///
    /// ⚠️ The layout cascade below is a hand-rolled twin of `assignment.rs::choose_layout`
    /// — one prettier function answered twice, once for a declarator and once for every
    /// other assignment. It reaches arms `choose_layout` does not (the three chain shapes),
    /// so the two cannot simply be merged — but they DRIFT, and a `chooseLayout` fact added
    /// to one belongs in both.
    pub(in crate::printer) fn build_declarator_init_doc(
        &self,
        inputs: &DeclaratorInitInputs<'_, '_>,
        value: &dyn Fn() -> DocId,
    ) -> DocId {
        let d = self.d();
        let &DeclaratorInitInputs {
            declarator,
            init,
            decl_start,
            id_doc,
            can_break_left,
            gap:
                DeclaratorEqGap {
                    id_end,
                    equals_pos,
                    init_start,
                    has_comments_before_eq,
                    has_comments_after_eq,
                },
            should_break,
            is_first,
        } = inputs;
        let mut parts: DocBuf = DocBuf::new();
        // Comments after `=` all stay after `=`, matching prettier — a JSDoc
        // cast (`= /** @type {T} */ (expr)`) keeps its parens via the
        // `JsdocCast` node, so its comment lives inside the init expression and
        // never reaches this gap.
        let rhs_comments_start = equals_pos + 1;

        // Helpers for LHS doc handling. Most branches use id_doc as-is;
        // some rebuild it (break-lhs wrapping type, fluid non-wrapping type).
        // Comments before `=` are always appended after the LHS. (Only block
        // comments reach here — a before-`=` *line* comment took the
        // continuation `continue` path above.)
        // The LHS every arm emits: the binding, plus the before-`=` comments appended to it.
        // ONE doc rather than a push-pair, because the fluid arms need it as a single doc
        // (it goes inside their group) and no other arm can tell the difference — two
        // spellings of one append is how they drift. Only block comments reach here; a
        // before-`=` *line* comment took the caller's continuation path.
        let lhs_doc_with_comments = |lhs_doc: DocId| -> DocId {
            if has_comments_before_eq {
                d.concat(&[
                    lhs_doc,
                    self.build_inline_comments_between_doc(id_end, equals_pos),
                ])
            } else {
                lhs_doc
            }
        };

        // Build optional inline block comment doc between `=` and init.
        // These are comments like `const x = /* comment */ expr` that should be
        // part of the RHS doc in assignment layout decisions. Line comments are
        // handled separately (mandatory break path).
        let rhs_block_comment_doc = if has_comments_after_eq {
            self.build_comments_between_filtered_opt(
                rhs_comments_start,
                init_start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            )
        } else {
            None
        };

        // A declarator `=` is a value gap (`mark_jsdoc_cast_value_gap`). Marked
        // before any branch below builds the value; the flag is span-keyed, so it is
        // read wherever that build lands.
        self.mark_jsdoc_cast_value_gap(init);

        // Helper: build init doc with optional inline block comments prepended.
        // Comments use Trailing spacing (`/* comment */ `) so no extra space needed.
        let make_init_doc = |init_doc: DocId| -> DocId {
            if let Some(comment_doc) = rhs_block_comment_doc {
                d.concat(&[comment_doc, init_doc])
            } else {
                init_doc
            }
        };

        // A block run the author broke AFTER (`const y = /* c */⏎<value>`):
        // the shared `=` broke-after arm ([`Printer::broke_after_operator_rhs_doc`]
        // — the two-half rule, its declines, and what falls through live there).
        if rhs_block_comment_doc.is_some()
            && let Some(rhs_doc) =
                self.broke_after_operator_rhs_doc(rhs_comments_start, init_start, value)
        {
            parts.push(lhs_doc_with_comments(id_doc));
            parts.push(d.text(" ="));
            parts.push(rhs_doc);
            return d.concat(&parts);
        }

        // Check if RHS is a multiline string (line continuations)
        let is_multiline_string = is_multiline_string_literal(init, self.source);

        // Check if LHS triggers break-lhs layout:
        // 1. Complex type annotation - nested generics that should break internally
        // 2. Complex destructuring - >2 properties with defaults/non-shorthand
        // 3. Arrow function with breakable LHS (long type annotation)
        //
        // Example type annotation: `const x: Map<string, Array<number>> = getLongValue()`
        // Should break as:
        //   const x: Map<
        //     string,
        //     Array<number>
        //   > = getLongValue();
        //
        // Example destructuring: `const { a, b = 1, c } = obj`
        // Should break as:
        //   const {
        //     a,
        //     b = 1,
        //     c,
        //   } = obj;
        //
        // Example arrow with long type: `const fn: (x: number) => void = (x) => {}`
        // When type is long enough to wrap:
        //   const fn: (
        //     x: number,
        //   ) => void = (x) => {};
        let has_complex_type_annotation = self.id_has_complex_type_annotation(declarator.id);
        let has_complex_destructuring = self.id_has_complex_destructuring(declarator.id);
        let is_arrow_with_breakable_left =
            matches!(init, Expression::ArrowFunctionExpression(_)) && can_break_left;

        // Break-after-operator layout: group([left, " =", group(indent([line, right]))])
        // Used for fluid RHS or simple RHS when LHS can break.

        // Calls and imports with trailing comments expand internally and should not use fluid layout
        let is_call_with_trailing_comments = if let Expression::CallExpression(call) = init {
            call.arguments.last().is_some_and(|last_arg| {
                self.has_line_comments_between(last_arg.span().end, call.span.end)
            })
        } else {
            false
        };

        // Import expressions with trailing comments also expand internally
        // (handles `await import('./x' // comment)`)
        let is_import_with_trailing_comments = self.has_import_with_trailing_comments(init);

        // Call chains AND member-only chains with line comments should NOT be
        // treated as fluid / break-after-operator. The chain formatter breaks
        // internally at the comment location, so keep the chain with `=`
        // (otherwise it breaks after `=` too → double indent). E.g.
        // `const a = items // comment\n  .foo()` and `const b = foo.bar // c\n  .baz`.
        let has_line_comments_in_chain = self.has_line_comments_in_call_chain(init)
            || self.has_line_comments_in_member_chain(init);

        // Combined flag for expressions with trailing comments that expand internally
        let has_trailing_comment_expansion =
            is_call_with_trailing_comments || is_import_with_trailing_comments;

        // Common exclusion: layout strategies don't apply when the init
        // self-expands (object/array), has trailing comment expansion, or
        // has line comments in a chain — those need special handling.
        let is_layout_eligible = !is_self_expanding_value(init)
            && !has_trailing_comment_expansion
            && !has_line_comments_in_chain;

        // RHS expressions that should use break-after-operator layout.
        // Matches Prettier's shouldBreakAfterOperator: poorly breakable chains,
        // string literals, etc. These don't break well internally, so the
        // assignment breaks at `=` with group(indent([line, rightDoc])).
        //
        // ⚠️ **This chain is a hand-rolled twin of `assignment.rs::choose_layout`** —
        // one prettier function answered twice, once for `const x = …` and once for
        // every other assignment. It reaches arms `choose_layout` does not (the three
        // chain shapes below), so the two cannot simply be merged — but they DRIFT, and
        // the sequence arm is the proof: `choose_layout` has had it from the start while
        // this list did not, so `const a = (a, b)` hung its operands off the `=` column.
        // A `chooseLayout` fact added to one belongs in both.
        let should_break_after_op_rhs = (is_module_path_fluid_call(init, self.source)
            || is_pure_property_chain(init)
            || is_poorly_breakable_chain(init, self.source, PRINT_WIDTH, self.comments)
            || is_string_literal(init)
            // A SEQUENCE init breaks after the `=` and lays its operands out under
            // one indent, prettier's own `shouldBreakAfterOperator` switch arm —
            // the same fact `choose_layout` states for the assignment-RHS twin.
            // Without it the sequence's internal break satisfies the fluid layout's
            // fits() and the operands hang off the `=` column instead.
            || matches!(init, Expression::SequenceExpression(_))
            || matches!(init, Expression::RegexLiteral(_)))
            && is_layout_eligible;

        // Decorated class expression → break after operator, each decorator
        // on its own line (`const C =\n\t@dec\n\tclass {}`).
        let is_decorated_class_expr = is_layout_eligible
            && matches!(init, Expression::ClassExpression(c) if class_expr_has_decorators(c));

        // Single-call member chains with complex args (arrows, objects, arrays):
        // Use TRUE fluid layout to break at `=` only when necessary.
        // E.g., `const x = a.b.c.filter((x) => ...)` breaks at `=` if > print_width
        let is_single_call_member_chain = is_call_on_member_chain(init) && is_layout_eligible;

        // Regex-rooted member chain calls: /regex/.exec(b)
        // Prettier returns "fluid" layout (its default) because regex roots are NOT
        // accepted by isPoorlyBreakableMemberOrCallChain (only Identifier/ThisExpression).
        // Our is_poorly_breakable_chain similarly rejects regex roots. Route to fluid
        // so fits() can decide whether to break at `=` or let the call expand args.
        let is_regex_chain_call = is_regex_root_chain(init) && is_layout_eligible;

        // Member-only chains on literal bases: 'string'.length, `template`.length
        // These need Fluid layout so the assignment can break at `=` when the
        // literal base exceeds print_width on the assignment line.
        let is_literal_member = is_literal_member_chain(init) && is_layout_eligible;

        // Expressions that need break-after-operator layout:
        // group([left, " =", indent([line, right])])
        // For binary/logical expressions, breaking happens at operators within the RHS,
        // and the entire RHS is indented together after `=`.
        //
        // Excludes logical expressions with inline-able RHS (non-empty object/array).
        // Those use default layout so the RHS self-expands:
        //   `const x = foo || { a: 1 }` not `const x =\n  foo || {a: 1}`
        // Prettier ref: assignment.js:199, binaryish.js:361
        let is_non_inline_binary = if let Expression::BinaryExpression(binary) = init {
            !should_inline_logical_expression(binary)
        } else {
            false
        };
        let needs_break_after_op_layout =
            (is_non_inline_binary || conditional_should_break_after_op(init)) && is_layout_eligible;

        // Member-chain call (a.fn(...)) where the call head fits within print_width:
        // Use default layout and let the call expand its own args rather than breaking
        // at `=`. E.g., `const {a, b} = vi.mocked(longArg)` with short LHS keeps
        // `= vi.mocked(` on line 1 and expands the arg — matching Prettier's behavior.
        // Only fires when call head (decl_start to callee_end + "(") fits in print_width.
        // is_single_call_on_member_chain guarantees CallExpression
        let is_expandable_member_call = if let Expression::CallExpression(call) = init
            && is_single_call_on_member_chain(init)
        {
            // Include actual source indentation (JS nesting) in the width check.
            // Without this, deeply-nested declarations would incorrectly use
            // default layout even when the call head exceeds print_width.
            let indent_visual = self.source_indent_visual(decl_start);
            let call_head_width =
                indent_visual + (call.callee.span().end as usize - decl_start as usize) + 1; // +1 for "(" after callee
            call_head_width < PRINT_WIDTH
        } else {
            false
        };

        // A comment the initializer *owns* (a JSDoc cast, a bundler annotation) is
        // glued to its first token and travels inside its doc, so the gap probes
        // above cannot see it. It is still on the page and still decides the `=`
        // layout — this declarator builds its own layout rather than routing
        // through `build_assignment_layout`, so it applies the rule itself. Both
        // halves come off one lookup; see `owned_leading_comment_effect`.
        let owned_comment_effect = self.owned_leading_comment_effect(init);

        let is_break_after_op_rhs = should_break_after_op_rhs
            || needs_break_after_op_layout
            || is_decorated_class_expr
            // An indentable owned comment hangs the value.
            || owned_comment_effect == Some(OwnedCommentEffect::Hangs);

        // The other half: a *preserved* multi-line comment the initializer owns
        // ends the `=` line inside itself, so no width-decided break at `=` is
        // meaningful and the plain `= value` form is the layout
        // (`const a = /* line1⏎line2 */ x;`). Without this the fluid branches broke
        // at `=` on the comment's own `literalline`s.
        let init_pinned_to_eq = owned_comment_effect == Some(OwnedCommentEffect::Pins);

        // Breakable LHS (destructuring patterns) with non-self-expanding RHS:
        // Use fluid layout so the printer breaks at `=` before expanding the
        // destructuring pattern. Matches Prettier's `canBreak(leftDoc) → "fluid"`.
        // E.g., `const {a, b, c} = resolve(x, y, z)` breaks after `=`, not inside `{}`
        //
        // Excludes break-after-operator RHS (binary, conditional, strings, chains) —
        // those go through needs_break_after_operator with their own layout.
        // In Prettier, shouldBreakAfterOperator() handles those before the canBreak fallback.
        //
        // Excludes is_expandable_member_call: when the call head fits, the call's own
        // arg-expansion handles line breaking via default layout.
        let needs_fluid_for_breakable_lhs = can_break_left
            && is_layout_eligible
            && !should_break
            && !is_break_after_op_rhs
            && !is_expandable_member_call
            && !init_pinned_to_eq;

        // Type assertion calls with LHS type annotation need special fluid handling
        // (handled separately below because they need non-wrapping LHS type)
        let is_type_assertion_with_lhs_type = is_type_assertion_call(
            init,
            self.source,
            PRINT_WIDTH,
        ) && matches!(&declarator.id, Expression::Identifier(id) if id.type_annotation().is_some());

        let is_simple_rhs_with_breakable_lhs = can_break_left && is_simple_self_expanding(init);

        // `should_break` (a multi-declarator with initializers) withholds the
        // width-decided break at `=` because the declarators are hardline-separated
        // already — but an owned comment that HANGS is not width-decided. Its break
        // is inside the value's doc whatever this layout picks, so withholding the
        // hang only strands what follows the comment at the declarator list's own
        // indent: for a cast, a form the next pass reads as mid-line and collapses,
        // leaving that authoring no fixed point at all. The gap-emitted spelling of
        // the same comment already hangs here (`build_eq_comment_break_rhs`, the
        // first branch below, which `should_break` does not gate), so this is the
        // owned half catching up to it
        // (`multiple/value_own_line_comment_hang_prettier_divergence`).
        let needs_break_after_operator = (!should_break
            || owned_comment_effect == Some(OwnedCommentEffect::Hangs))
            && (is_break_after_op_rhs || is_simple_rhs_with_breakable_lhs)
            && !d.will_break(id_doc)
            && !has_complex_type_annotation
            && !has_complex_destructuring
            && !is_arrow_with_breakable_left;

        // A curried chain whose heads trigger `arrow_chain_should_break` breaks
        // after `=` unconditionally; every other curried chain goes fluid below.
        // ⚠️ This pair is the declarator's hand-rolled twin of `choose_layout`'s
        // `Fluid` / `BreakAfterOperator` arms — see the ⚠️ on `build_assignment_layout`.
        let is_curried_arrow = is_curried_arrow_chain_that_breaks(init);

        if has_comments_after_eq
            && let Some(rhs) = self.build_eq_comment_break_rhs(equals_pos, init_start, " =", value)
        {
            // A comment after `=` forces a break (line comment → partition;
            // own-line / multiline block → break-after-operator hang). Shared
            // with the for-loop init clause.
            parts.push(lhs_doc_with_comments(id_doc));
            parts.push(rhs);
        } else if is_multiline_string {
            // Multiline string with no comment forcing a break: mandatory break
            // after `=`. An inline block glued to `=` trails it on that line.
            parts.push(lhs_doc_with_comments(id_doc));
            parts.push(d.text(" ="));
            if has_comments_after_eq
                && let Some(inline) =
                    self.build_inline_comments_between_doc_opt(equals_pos + 1, init_start)
            {
                parts.push(inline);
            }
            parts.push(d.indent_hardline(value()));
        } else if is_curried_arrow {
            // Mandatory break after `=`; the arrow printer stacks the heads under it.
            // ⚠️ Deliberately does NOT set `ArrowChainContext::AssignmentRhs`, unlike
            // the arm below — and it is only equivalent to setting it because
            // `should_use_arrow_chain_layout` declines a `shouldBreakChain` chain in
            // exactly that context, precisely so this `=` can own the break instead.
            // `build_assignment_layout` sets the context for EVERY curried chain and
            // leans on the same decline; if that decline ever moves, both sites move.
            parts.push(lhs_doc_with_comments(id_doc));
            parts.push(d.text(" ="));
            parts.push(d.indent_hardline(value()));
        } else if is_curried_arrow_chain(init) {
            // Every other curried chain: fluid break after `=`. The chain's
            // signature heads break only when they don't fit on the operator
            // line; a hugging body otherwise expands in place. The context tells
            // the arrow printer to use the assignment-RHS chain layout.
            let init_doc = self.build_with_arrow_chain_context(
                crate::printer::ArrowChainContext::AssignmentRhs,
                || make_init_doc(value()),
            );
            parts.push(build_fluid_assignment_doc(
                d,
                lhs_doc_with_comments(id_doc),
                init_doc,
            ));
        } else if (has_complex_type_annotation
            || has_complex_destructuring
            || is_arrow_with_breakable_left)
            && (should_break || is_first)
        {
            // Break-lhs layout: LHS breaks internally, `=` stays on same line with RHS
            // Only applies to first declarator or multi-declarator with breaks
            //
            // For complex type annotations, rebuild with wrapping type.
            // Complex destructuring and arrow with breakable left already have correct id_doc.
            if has_complex_type_annotation && let Expression::Identifier(ident) = &declarator.id {
                parts.push(lhs_doc_with_comments(self.build_typed_identifier_doc(
                    ident,
                    declarator.definite,
                    true, // wrap_type
                )));
            } else if has_complex_destructuring {
                // Strip the outer group from the destructuring id_doc so it
                // participates in the outer group's fit check. Without this,
                // the destructuring group evaluates independently via fits()
                // and stays flat even when the full line exceeds print_width.
                // Prettier's break-lhs does not wrap leftDoc in an extra group.
                parts.push(lhs_doc_with_comments(d.unwrap_group(id_doc)));
            } else {
                parts.push(lhs_doc_with_comments(id_doc));
            }

            // Add ` = rightDoc` (right side grouped)
            parts.push(d.text(" = "));
            let init_doc = make_init_doc(value());
            parts.push(d.group(init_doc));
        } else if is_type_assertion_with_lhs_type
            || is_single_call_member_chain
            || needs_fluid_for_breakable_lhs
            || is_regex_chain_call
            || is_literal_member
        {
            // Fluid layout for specific RHS patterns: break after `=` only
            // when the full line exceeds print_width. Type assertion case
            // rebuilds LHS with non-wrapping type annotation.
            let fluid_id_doc = if is_type_assertion_with_lhs_type
                && let Expression::Identifier(ident) = &declarator.id
            {
                self.build_typed_identifier_doc(
                    ident,
                    declarator.definite,
                    false, // non-wrapping
                )
            } else {
                id_doc
            };
            let init_doc = make_init_doc(value());
            parts.push(build_fluid_assignment_doc(
                d,
                lhs_doc_with_comments(fluid_id_doc),
                init_doc,
            ));
        } else if needs_break_after_operator {
            // Break-after-operator layout for binary/conditional expressions:
            // Structure: [" =", group(indent([line, init]))]
            //
            // The init IS inside the group with the line. This allows the binary/conditional
            // expression to control its own breaking at operators. The entire RHS is
            // indented together after the `=` break.
            parts.push(lhs_doc_with_comments(id_doc));
            parts.push(d.text(" ="));
            let init_doc = make_init_doc(value());
            parts.push(hang_after_operator(d, init_doc));
        } else if is_layout_eligible && !is_simple_value(init) && !init_pinned_to_eq {
            // Fluid layout (default for layout-eligible values)
            //
            // Matches prettier's chooseLayout default: when no special layout
            // applies, use fluid so the marker can break at `=` only if needed,
            // while allowing the RHS to break internally first.
            let init_doc = make_init_doc(value());
            parts.push(build_fluid_assignment_doc(
                d,
                lhs_doc_with_comments(id_doc),
                init_doc,
            ));
        } else {
            parts.push(lhs_doc_with_comments(id_doc));
            parts.push(d.text(" = "));
            let init_doc = make_init_doc(value());
            parts.push(init_doc);
        }
        d.concat(&parts)
    }

    /// Build a variable initializer value, wrapping it in parens for the value
    /// position when needed (`const x = (a = b)`) — but NOT double-wrapping when
    /// `build_expression_doc_with_paren_comments` already added its own pair around a
    /// trailing comment (`const y = (a = b // c)` stays single, not `((a = b // c))`).
    /// The single paren then matches the assignment-RHS rendering.
    ///
    /// `position_parens` is asked ONCE and answers two questions that must agree — the
    /// wrap below, and whether the shell builder holds the comment inside the pair
    /// ([`Printer::shell_value_keeps_own_parens`]). Asking it separately on each side is
    /// how the pair gets doubled or dropped.
    ///
    /// The paren decision carries the for-header rule via `self.in_for_init`: a
    /// statement-level `const x = b in c` lexically under a for-header init (e.g. in a
    /// nested function body) parenthesizes the `in` like every other position there —
    /// this path's shell builder is the `StatementTerminator` tail, whose `wrap_in` is a
    /// no-op, so the caller is the only one who can supply that pair.
    ///
    /// ⚠️ A for-header's *own* declarator is built in `build_for_init_doc` and passes
    /// `false` for the same argument, deliberately: its `ForClauseSeparator` tail DOES
    /// apply `wrap_in`, so reading the flag there would spell one wrap twice. The
    /// asymmetry is the point — don't "fix" it into agreement.
    ///
    /// `frozen` is the value-head freeze this position resolved (`=`→initializer, per
    /// [`Printer::value_head_frozen_span`]): the whole initializer prints verbatim, with
    /// the same clarity parens the ordinary path would supply — the position's own
    /// [`ParenContext`], so the two forms cannot disagree about the shell. Every layout
    /// branch below routes its value through here, so the freeze is stated once.
    fn build_init_value_doc(
        &self,
        init: &Expression<'_>,
        boundary_end: u32,
        frozen: Option<Span>,
    ) -> DocId {
        // Declarator init — an `ancestorNameMap` value position. A frozen value takes the
        // same shell (its slice replaces the expression doc, nothing else); only the
        // ternary mark is unfrozen-only, since a verbatim slice takes no layout from it.
        if frozen.is_none() {
            self.mark_ternary_extra_indent(init);
        }
        // A declarator initializer is the second position prettier's call-object clause
        // names ([`Printer::mark_member_call_tail_operand`]). ⚠️ A for-header's own
        // declarator does NOT reach here — it is built in `build_for_init_doc` and marks
        // itself; `VariableDeclarator` is `VariableDeclarator` to prettier either way, so
        // unlike the `[~In]` asymmetry above, the two DO have to agree here.
        self.mark_member_call_tail_operand(init);
        let position_parens =
            needs_parens(init, ParenContext::VariableInit, self.in_for_init.get());
        let inner = match frozen {
            Some(frozen) => {
                self.build_frozen_value_shell_doc(init, frozen, boundary_end, position_parens)
            }
            None => {
                self.build_expression_doc_with_paren_comments(init, boundary_end, position_parens)
            }
        };
        self.wrap_value_position_parens(init, boundary_end, position_parens, inner)
    }

    /// Build a doc for a variable binding pattern with optional definite assignment assertion.
    ///
    /// For identifiers with `definite: true`, builds doc for `name!: type` instead of `name: type`.
    /// Uses wrapping type annotations so TypeReference type arguments break internally when needed.
    fn build_variable_binding_doc(&self, id: &Expression<'_>, definite: bool) -> DocId {
        if definite {
            if let Expression::Identifier(ident) = id {
                self.build_typed_identifier_doc(ident, true, true)
            } else {
                // Destructuring patterns don't support definite assignment
                self.build_expression_doc(id)
            }
        } else if let Expression::Identifier(ident) = id {
            self.build_identifier_doc_with_wrapping_type(ident)
        } else {
            self.build_expression_doc(id)
        }
    }

    /// Build doc for an identifier with type annotation, configurable wrapping.
    ///
    /// - `definite`: include `!` after name
    /// - `wrap_type`: use wrapping type annotation (breaks internally) vs non-wrapping (stays on one line)
    fn build_typed_identifier_doc(
        &self,
        ident: &internal::Identifier<'_>,
        definite: bool,
        wrap_type: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = smallvec![self.identifier_name_doc(ident)];

        // Compute name_end for comment extraction
        let search_end = ident
            .type_annotation()
            .map_or(ident.span.end, |ta| ta.span.start);
        let raw_name_end = analysis::skip_identifier_at(
            self.source.as_bytes(),
            ident.span.start as usize,
            search_end as usize,
        ) as u32;
        let mut after_modifier = raw_name_end;

        if definite {
            after_modifier = self.push_modifier_marker_doc(&mut parts, after_modifier, b'!');
        }
        if ident.optional {
            after_modifier = self.push_modifier_marker_doc(&mut parts, after_modifier, b'?');
        }
        if let Some(type_ann) = ident.type_annotation() {
            // `: type` annotation, handling a before-`:` comment between the binding
            // name (and any `!`/`?`) and `:` — line → indented continuation, block →
            // inline before `:`.
            parts.push(self.build_binding_type_annotation_doc(after_modifier, type_ann, wrap_type));
        }
        d.concat(&parts)
    }

    /// Build a Doc for a variable declaration statement
    ///
    /// Handles declare, definite assignment (!), type annotations, and multiple declarators.
    /// Follows prettier's rule: if any declarator has an initializer, break to multiple lines.
    /// `emit_semicolon` is `false` only for embedders that supply their own
    /// terminator — Svelte's `{const …}`/`{let …}` tags close with `}` and drop
    /// the `;` (a bare `{let a}` is the lone exception, which passes `true`).
    pub(crate) fn build_variable_declaration_doc(
        &self,
        decl: &internal::VariableDeclaration<'_>,
        emit_semicolon: bool,
        clause_tail: Option<u8>,
    ) -> DocId {
        let d = self.d();
        let mut prefix: DocBuf = DocBuf::new();

        let first_decl_start = decl.declarations[0].span.start;

        // The header keyword, word by word: an optional `declare` modifier plus the
        // kind (`await using` is two words). Every gap *between* those words is a
        // position an author can comment in, so the words are located rather than
        // measured — measuring skips the interior gaps and drops what's in them.
        let mut words: SmallVec<[&'static str; 3]> = SmallVec::new();
        if decl.declare {
            words.push("declare");
        }
        words.extend_from_slice(decl.kind.words());
        // The keyword→first-declarator gap. A *line* comment here indents the whole
        // continuation one level (uniform declaration-header rule); block/no-comment
        // cases stay inline. The leading space is supplied by the gap helper below.
        let (keyword_doc, keyword_end) =
            self.build_keyword_words_doc(&words, decl.span.start, first_decl_start);
        prefix.push(keyword_doc);

        // Everything after the gap is collected into `parts` (the continuation).
        let mut parts = DocBuf::new();

        let is_multi_declarator = decl.declarations.len() > 1;
        let has_any_init = decl.declarations.iter().any(|d| d.init.is_some());
        let should_break = is_multi_declarator && has_any_init;

        // The continuation indent the broken declarators carry. Normally explicit `INDENT`
        // text, because these declarators aren't wrapped in a `d.indent()` — but when the
        // keyword→first-declarator gap breaks, `build_keyword_to_name_continuation` wraps the
        // whole continuation in one, and emitting both puts every declarator after the first
        // two levels deep.
        let continuation_indent = if self.keyword_gap_breaks(keyword_end, first_decl_start) {
            d.empty()
        } else {
            d.text(INDENT)
        };

        // The Rule A gap anchors, in the shared closure form `list_item_frozen` takes. The
        // first declarator's gap opens past the keyword, so a directive written between
        // `const`/`let`/`var` and it freezes it like any other member; the header emitter
        // keeps that directive on its own line ([`Printer::build_header_comment_run`]) —
        // flush against the keyword it would be inert and the freeze would be lost on the
        // second pass.
        let item_span = |j: usize| decl.declarations[j].span;

        // When breaking to multiple lines, multiline objects/arrays get extra indentation
        // Use save/restore pattern for nested multi-declarator safety
        let old_indent_depth = self.declaration_indent_depth.get();
        if should_break {
            self.declaration_indent_depth.set(old_indent_depth + 1);
        }

        // Build continuation declarators for the non-break case (no initializers)
        // These get wrapped in indent() so when the group breaks, they get continuation indent
        let mut rest_parts = DocBuf::new();

        // Set top-level assignment flag for chain detection
        // Short 2-segment assignment chains in variable declarations should not use chain formatting
        //
        // SAVED, not just set: a declarator's initializer can contain another declaration
        // (`const a = () => { const b = 1; }, c = …`), and `build_block_statement_doc`'s
        // save/restore covers `is_expression_statement` only. Restoring the constant `false`
        // therefore left every declarator AFTER such an initializer building in the wrong
        // context — an authoring-ORDER dependence. Reached 7,314× over ~23k real files, and
        // observably neutral there (0 movers), so this is a correct-by-construction fix
        // rather than a repair: the stale value is never read back today.
        let prev_top_level_assignment = self.in_top_level_assignment.replace(true);

        // Declarators
        for (i, declarator) in decl.declarations.iter().enumerate() {
            if i > 0 {
                let prev_end = decl.declarations[i - 1].span.end;
                let curr_start = declarator.span.start;

                // Check for comments between declarators
                let has_line_comment = self.has_line_comments_between(prev_end, curr_start);
                let has_gap_comment = self.has_comments_to_emit_between(prev_end, curr_start);
                // The declarator-separating comma. A block comment keeps the author's
                // side of it: before → trails the previous init; after → leads the next
                // declarator. (Only consulted when `has_gap_comment`.)
                let comma_pos = self.comma_between(prev_end, curr_start);

                if should_break {
                    if has_line_comment {
                        // Line comment(s) between declarators: the comma must go before
                        // the first line comment, block comments go before the comma.
                        // e.g. `a = 1 /* c1 */,\n// c2\nb = 2` or `a = 1, // c1\n// c2\nb = 2`.
                        // The gap owns its own break, so it carries the continuation indent.
                        self.push_inter_item_line_comment_gap(
                            &mut parts,
                            prev_end,
                            comma_pos,
                            curr_start,
                            continuation_indent,
                        );
                    } else {
                        // Block comment(s) before the comma trail the previous init
                        // (`a = 1 /* c */,`); after-comma comments lead the next
                        // declarator (below the break). Prettier preserves the side.
                        if has_gap_comment {
                            self.push_before_comma_blocks(&mut parts, prev_end, comma_pos);
                        }
                        parts.push(d.text(","));
                        // A stranded after-comma block (on the comma's line, but a
                        // newline before the next declarator) trails the comma —
                        // preserving the author's placement (prettier relocates it
                        // before the comma). Emitted before the break below.
                        self.push_stranded_after_comma_blocks(&mut parts, comma_pos, curr_start);
                        // Break to new line with indentation for next declarator
                        parts.push(d.hardline());
                        parts.push(continuation_indent);
                        if has_gap_comment {
                            // After-comma block comment(s) lead the next declarator. A
                            // stranded block already trailed the comma above.
                            let comments: CommentVec<'_> =
                                comments_to_emit_in_range(self.comments, comma_pos, curr_start)
                                    .filter(|c| {
                                        !self
                                            .is_stranded_after_comma_block(c, comma_pos, curr_start)
                                    })
                                    .collect();
                            self.push_leading_comment_run(
                                &mut parts,
                                comments.iter().copied(),
                                curr_start,
                                LeadingGlue::Adjacent,
                                continuation_indent,
                            );
                        }
                    }
                } else {
                    // Non-break case (no initializers): every continuation declarator
                    // lives in `rest_parts`, which is wrapped in `d.indent()` below — so
                    // the continuation indent comes from the doc tree and each break here
                    // carries an empty one. The comma goes there too: nothing breaks
                    // before it, so the enclosing indent is inert on it.
                    if has_line_comment {
                        // A line comment runs to EOL, so the comma must precede it and the
                        // next declarator must drop below — the soft `line` used for the
                        // comment-free case would let the comment absorb both.
                        self.push_inter_item_line_comment_gap(
                            &mut rest_parts,
                            prev_end,
                            comma_pos,
                            curr_start,
                            d.empty(),
                        );
                    } else {
                        // Block comments keep their side of the comma (preserve position).
                        if has_gap_comment {
                            self.push_before_comma_blocks(&mut rest_parts, prev_end, comma_pos);
                        }
                        rest_parts.push(d.text(","));
                        // Soft break for declarations without initializers
                        rest_parts.push(d.line());
                        if has_gap_comment {
                            self.push_leading_comment_run(
                                &mut rest_parts,
                                comments_to_emit_in_range(self.comments, comma_pos, curr_start),
                                curr_start,
                                LeadingGlue::Adjacent,
                                d.empty(),
                            );
                        }
                    }
                }
            }

            // A continuation declarator with no initializer is the one case that lives in
            // `rest_parts` (wrapped in `d.indent()` after the loop); everything else feeds
            // `parts`. Named once, then reused by the freeze arm and the plain-`id_doc` arms.
            let goes_in_parts = should_break || i == 0;

            // Rule A: an own-line directive in an inter-declarator gap freezes the FOLLOWING
            // declarator over its own node span — the annotation, `=`, and initializer all
            // ride inside it; the separating `,` is parent-owned and was emitted above.
            if self.list_item_frozen(keyword_end, &item_span, i) {
                let frozen_doc = self.build_frozen_node_doc(declarator.span);
                if goes_in_parts {
                    parts.push(frozen_doc);
                } else {
                    rest_parts.push(frozen_doc);
                }
                continue;
            }

            // Build id doc once for reuse and analysis
            let id_doc = self.build_variable_binding_doc(declarator.id, declarator.definite);

            // Check if id doc can break (contains line elements like type annotations that wrap)
            // This matches Prettier's `canBreak(leftDoc)` check
            let can_break_left = d.can_break(id_doc);

            if !should_break && i > 0 {
                // Non-break continuation declarators go to rest_parts (never have inits)
                rest_parts.push(id_doc);
            }

            // Initializer with comment handling around =
            if let Some(init) = &declarator.init {
                let mut id_end = declarator.id.span().end;
                let init_start = init.span().start;
                // With definite assignment but no type annotation (`let a! = x`), the id
                // span excludes the `!`; advance past it so comments between the name and
                // `!` (already emitted inside the id doc) aren't re-emitted before `=`.
                if declarator.definite
                    && let Expression::Identifier(ident) = &declarator.id
                    && ident.type_annotation().is_none()
                    && let Some(bang_pos) =
                        self.find_char_outside_comments(id_end, init_start, b'!')
                {
                    id_end = bang_pos + 1;
                }
                // Zero-comment fast gate: one binary search over the whole name→value
                // gap. The overwhelming common case (no comment around `=`) then skips
                // locating `=` — a byte-by-byte source scan — and both the before-`=`
                // and after-`=` comment probes, falling through to the plain `id = init`
                // layout below. Sound because comments are start-sorted + disjoint and
                // both gap sub-ranges lie within `[id_end, init_start]`, so an empty
                // whole-gap window makes every sub-query provably empty. Canonical
                // reference: `build_params_doc_with_comments`.
                let gap_has_comments = self.has_comments_on_page_between(id_end, init_start);
                let (equals_pos, has_comments_before_eq, has_comments_after_eq) =
                    if gap_has_comments {
                        let eq = self.find_equals_position(id_end, init_start);
                        (
                            eq,
                            self.has_comments_to_emit_between(id_end, eq),
                            self.has_comments_to_emit_between(eq + 1, init_start),
                        )
                    } else {
                        // No gap comment ⇒ the `=` position is never consulted (its only
                        // uses bound comment ranges, all empty here); the value below is
                        // a never-read sentinel that keeps the gated call sites' ranges
                        // trivially empty.
                        (init_start, false, false)
                    };

                // The `=`→initializer value head: an own-line directive there freezes the
                // whole initializer. Rides the gap probe above — a directive is a comment,
                // so a comment-free gap provably holds none.
                let init_frozen = if has_comments_after_eq {
                    self.value_head_frozen_span(equals_pos + 1, init.span())
                } else {
                    None
                };

                // A line comment between the binding and `=` keeps the comment in place
                // and drops `= value` to a continuation line indented one level (preserve
                // — lossless when a second comment also trails the statement; prettier
                // relocates it to end-of-statement and merges the two onto one line —
                // conformance_prettier_ts_comments.md §Comment relocation). Bypasses the
                // assignment-layout selection below; value built lazily so the common
                // no-comment path is unaffected. Init declarators always feed `parts`
                // (the comma/separator is handled above, the `;` after the loop), so a
                // plain push + `continue` is safe.
                if has_comments_before_eq
                    && let Some(cont) = self.build_initializer_line_continuation(
                        id_end,
                        equals_pos,
                        ContinuationValue::Expression(init),
                        || {
                            // The declarator's own value builder, exactly as every other
                            // layout branch below spells it: this arm changes where the `=`
                            // and its value SIT, not what the value is. Building the bare
                            // expression here instead dropped the position's clarity parens
                            // (`const a // c⏎= b = c`) and skipped the freeze outright, so a
                            // `prettier-ignore` in the gap silently normalized its value.
                            let value_doc =
                                self.build_init_value_doc(init, declarator.span.end, init_frozen);
                            self.prepend_rhs_comments(value_doc, equals_pos + 1, init_start)
                        },
                    )
                {
                    parts.push(id_doc);
                    parts.push(cont);
                    continue;
                }

                parts.push(self.build_declarator_init_doc(
                    &DeclaratorInitInputs {
                        declarator,
                        init,
                        decl_start: decl.span.start,
                        id_doc,
                        can_break_left,
                        gap: DeclaratorEqGap {
                            id_end,
                            equals_pos,
                            init_start,
                            has_comments_before_eq,
                            has_comments_after_eq,
                        },
                        should_break,
                        is_first: i == 0,
                    },
                    &|| self.build_init_value_doc(init, declarator.span.end, init_frozen),
                ));
            } else if should_break || i == 0 {
                // No initializer: push id_doc directly
                parts.push(id_doc);
            }
        }

        // For non-break multi-declarator, add rest_parts wrapped in indent
        if !should_break && !rest_parts.is_empty() {
            parts.push(d.indent(d.concat(&rest_parts)));
        }

        // Comments between the last declarator and the `;`, with the `;` bound to the
        // declaration: a same-line block trails *after* it (`const x = 1 /* c */;` →
        // `const x = 1; /* c */`, prettier 3.9), a same-line line trails after it via
        // `line_suffix` (`const x = 1; // c`), an own-line comment drops to its own line
        // after it (`const x = 1;⏎// c`). See `push_semicolon_with_gap_comments`.
        if emit_semicolon {
            if let Some(last) = decl.declarations.last() {
                self.push_semicolon_with_gap_comments(
                    &mut parts,
                    last.span.end,
                    decl.span.end,
                    true,
                    clause_tail,
                );
            } else {
                parts.push(d.text(";"));
            }
        }

        // Restore context flags — the PREVIOUS values, not constants (see the set above).
        self.declaration_indent_depth.set(old_indent_depth);
        self.in_top_level_assignment.set(prev_top_level_assignment);

        let continuation = if should_break {
            // Multi-declarator with initializers: hardline breaks already inserted
            d.concat(&parts)
        } else if is_multi_declarator || has_any_init {
            // Group for width-based breaking (multi-declarator soft breaks or single with init)
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        };
        // A line comment in the keyword→declarator gap indents the continuation.
        prefix.push(self.build_keyword_to_name_continuation(
            keyword_end,
            first_decl_start,
            continuation,
        ));
        d.concat(&prefix)
    }
}
