// Type declaration printing (type aliases, interfaces, enums, namespaces, declare functions)
// plus shared entity-name helpers

use super::{Printer, build_entity_name_doc, is_effectively_empty_body};
use crate::ast::internal::{self, TSType};
use crate::printer::ignore::is_freeze_target;
use crate::printer::layout::{fluid_after_operator, hang_after_operator};
use crate::printer::statements::function::FunctionHeadModifier;
use crate::printer::types::helpers::{
    type_needs_parens_for_array_element, type_needs_parens_for_indexed_access_object,
    unwrap_parenthesized,
};
use crate::printer::types::{ArraySuffixLayout, TrailingBlock};
use crate::printer::{
    CommentFilter, CommentSpacing, CommentVec, ContinuationValue, HeritageKeyword, LeadingGlue,
    MemberBlankScan, MemberBody, MemberFloor, MemberFreeze, MemberSeam,
};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::arena::DocId;
use tsv_lang::doc::{DocBuf, GroupId};
use tsv_lang::source_scan::find_char_skipping_comments;

/// Check if a type is "generic" - i.e., has type parameters.
/// This matches prettier's `isGeneric` function in assignment.js.
fn is_generic_type(ts_type: &TSType<'_>) -> bool {
    match ts_type {
        TSType::Function(f) => f.type_parameters.is_some(),
        TSType::TypeReference(r) => r.type_arguments.is_some(),
        _ => false,
    }
}

/// Check if we should break before the conditional type in a type alias.
/// Returns true if either checkType or extendsType has type parameters.
/// This matches prettier's `shouldBreakBeforeConditionalType` in assignment.js.
fn should_break_before_conditional_type(conditional: &internal::TSConditionalType<'_>) -> bool {
    is_generic_type(conditional.check_type) || is_generic_type(conditional.extends_type)
}

/// Returns true if the type has its own internal breaking mechanism
/// (e.g., braces, brackets, parentheses) and should NOT break after `=`.
///
/// Takes `&Printer` for the one arm whose answer depends on comments — an array
/// suffix owns a break point only once it prints one; see
/// [`Printer::array_suffix_layout`].
fn type_has_internal_breaking(printer: &Printer<'_>, ts_type: &TSType<'_>) -> bool {
    match ts_type {
        TSType::TypeLiteral(_)
        | TSType::Mapped(_)
        | TSType::Tuple(_)
        | TSType::Function(_)
        | TSType::Constructor(_)
        // `import(...)` hugs the `=` like a call — its specifier path doesn't
        // break, and any comments expand the parens internally.
        | TSType::Import(_) => true,
        // `typeof import(...)` hugs the `=` for the same reason — the import
        // call inside the type query provides the internal break.
        TSType::TypeQuery(q) => {
            matches!(q.expr_name, internal::TSTypeQueryExprName::Import(_))
        }
        // TypeReference with type arguments has internal breaking via `<>`
        TSType::TypeReference(r) => r.type_arguments.is_some(),
        // A **parenthesized** array element brings its own delimiter, so `(…)[]` breaks
        // inside its parens exactly as `{…}` breaks inside its braces. Without this the
        // gate answered on width alone: the same `(| 'a' | 'b')[]` hugged `=` when the
        // break was width-driven and hung after `=` when a member comment forced it, which
        // is the disagreement `build_conditional_check_doc` names for its own union gate.
        // A narrow arm, not the recursion the wider enumeration wants (`Ref<…>[]`,
        // `keyof Ref<…>`, `Ref<…>['k']` are still missing) — that one needs a corpus A/B.
        // The first disjunct needs no comment carve-out: a parenthesized element always
        // breaks once it carries one, since a trailing line comment RETAINS the shell over
        // real hardlines ([`Printer::paren_retains_for_trailing_run`]) and a leading one
        // takes its own `hardline`. A flat-shell exclusion here would be needed only if a
        // trailing run were deferred out past the closer, leaving the shell
        // rendering flat while still claiming a break.
        // The suffix's own `[]` is the same argument once it holds a comment
        // (`string[⏎↹// c⏎]`): the break is inside a delimiter the array owns, so it hugs
        // `=` exactly as the empty tuple type `[…]` and the empty object `{…}` already do.
        // Comment-free input answers `Fused` there, so this disjunct cannot move any input
        // the old enumeration covered. It reaches past the bare element to the one shape
        // the first disjunct misses — an element the author left bare that the printer
        // parenthesizes anyway (`typeof x /* c */[]`), where the AST holds no
        // `Parenthesized` node to match on.
        TSType::Array(a) => {
            matches!(a.element_type, TSType::Parenthesized(_))
                || matches!(printer.array_suffix_layout(a), ArraySuffixLayout::Split { .. })
        }
        // An indexed access whose index→`]` gap holds a line comment breaks inside the
        // brackets it owns, exactly as the array suffix's `[]` does above — so it hugs
        // `=` like the tuple / type-literal / type-argument siblings, which all keep an
        // internally-breaking value on the `=` line in both formatters. Comment-free
        // input never breaks here, so this disjunct cannot move any input the old
        // enumeration covered.
        TSType::IndexedAccess(i) => {
            printer.has_line_comments_between(i.index_type.span().end, i.span.end)
        }
        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Build a doc for type alias declaration with proper line breaking
    ///
    /// For union types that don't fit on one line:
    /// ```text
    /// type VeryLongTypeName =
    ///     | Type1
    ///     | Type2
    ///     | Type3;
    /// ```
    ///
    /// For intersection types that don't fit on one line:
    /// ```text
    /// type VeryLongTypeName = FirstType &
    ///     SecondType &
    ///     ThirdType;
    /// ```
    pub(super) fn build_type_alias_declaration_doc(
        &self,
        decl: &internal::TSTypeAliasDeclaration<'_>,
        clause_tail: Option<u8>,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![];
        // The header keyword, word by word — `declare`'s gap before `type` is a
        // position of its own and must not be folded into the keyword→name scan below.
        let (keyword_doc, keyword_end) = self.build_declaration_head_doc(
            decl.declare,
            &["type"],
            decl.span.start,
            decl.id.span.start,
        );
        parts.push(keyword_doc);
        // Comments between keyword and name: `type /* c */ A = string`
        parts.push(d.text(" "));
        if let Some(comments) = self
            .build_inline_comments_between_doc_trailing_space_opt(keyword_end, decl.id.span.start)
        {
            parts.push(comments);
        }
        parts.push(self.identifier_name_doc(&decl.id));

        // Check if type parameters are complex (>1 param with constraints/defaults)
        // Complex type params use break-lhs layout: params break, not the RHS
        let has_complex_params = self.type_alias_has_complex_params(decl.type_parameters.as_ref());

        // Compute `=` position early so we can use it as comment boundary
        let header_end = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.end);
        let type_start = decl.type_annotation.span().start;
        let eq_pos = self.find_equals_position(header_end, type_start);

        // Comments between name and type params: `type A/* c */ <T> = T`. The
        // name→`=` gap (no params) and type-params→`=` gap are handled below as
        // pre-`=` comments so they stay on the head side. Line comments get a
        // hardline to prevent absorbing type params as comment text.
        self.push_signature_head_comments(
            &mut parts,
            decl.id.span.end,
            decl.type_parameters.as_ref(),
            // A type alias has no parens, so with no type params the gap is empty.
            decl.id.span.end,
        );

        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Comments between the head (name + type params) and `=`. A single-line
        // block comment stays inline before `=` (`type A<X> /* c */ = B`); a line
        // comment (or a multiline block the author broke after) trails the head on
        // its line, then `= value` drops to a continuation line indented one level
        // — the uniform forced-continuation indent, the same shape as the other
        // before-`=` initializer sites (enum members, class properties, variable
        // declarators). tsv keeps these on the head side; prettier relocates them
        // after `=` (see conformance_prettier_ts_comments.md §Comment relocation).
        let pre_eq_forces_own_line = self.comments_force_own_line_between(header_end, eq_pos);

        if pre_eq_forces_own_line {
            let tail = self.build_type_alias_eq_value_doc(decl, eq_pos, has_complex_params, false);
            parts.push(self.build_continuation_indent(header_end, eq_pos, tail));
        } else {
            // Single-line block comments before `=` stay inline: `<head> /* c */ =`
            if let Some(block_doc) = self.build_comments_between_filtered_opt(
                header_end,
                eq_pos,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            ) {
                parts.push(block_doc);
            }
            parts.push(self.build_type_alias_eq_value_doc(decl, eq_pos, has_complex_params, true));
        }

        // Comments between the value and `;`, through the shared `;`-terminator seam:
        // a same-line block stays before the `;` (`type A = B /* c */;` — the operand
        // keeps it, the `import =` side of that axis), a same-line line comment floats
        // after it (`type A = B; // c`), and an own-line comment drops below the `;`
        // with any author blank above it intact.
        //
        // The last two shapes are why this is the shared emitter and not a kind-keyed
        // loop: such a loop answers "own-line?" with "is it a block?", so
        // it pulls an own-line block up onto the value's line and eats the blank —
        // disagreeing with every other `;` in the language (docs/comments.md §Trailing
        // and dangling runs).
        let value_end = decl.type_annotation.span().end;
        self.push_semicolon_with_gap_comments(
            &mut parts,
            value_end,
            decl.span.end,
            false,
            clause_tail,
        );

        d.concat(&parts)
    }

    /// Build the `=` token and the type-alias value, including any comments
    /// between `=` and the value. `lead_space` controls the leading space before
    /// `=` (true for the inline `... =` form, false when the caller has already
    /// broken the line, e.g. the pre-`=` comment continuation).
    /// A value whose own comment layout already breaks it internally, so the type-alias RHS
    /// keeps the value's head on the `=` line and lets the value hang its own tail — instead
    /// of *also* breaking after `=`, which would indent the whole thing a second level for
    /// nothing. The comment-driven sibling of the `conditional` / `type_has_internal_breaking`
    /// arms. Two shapes qualify:
    ///
    /// - a prefix type operator (`keyof` / `typeof`) whose keyword→operand gap holds a comment
    ///   that hangs the operand — the operator stays on `=`, the operand hangs beneath it;
    /// - a template-literal type with an expanding interpolation — the backtick stays on `=`,
    ///   the `${…}` expands beneath it (matching prettier).
    ///
    /// A **single-line** block comment (any position) or a **glued** multiline block collapses
    /// inline, so this returns false; the comment-free `=` layout then keeps the (short) value
    /// on the `=` line, and a long *comment-free* value still breaks after `=` (the
    /// hanging-indent arm). Each arm asks the same predicate the value's own printer asks
    /// (`comments_force_own_line_between` / `template_literal_type_breaks_for_comment`), so the `=` can't
    /// disagree with the value about whether the value breaks.
    fn value_owns_its_comment_break(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TypeOperator(o) => {
                let kw_end = o.span.start + o.operator.as_str().len() as u32;
                // Same deep window the operator's own builder measures: a
                // `keyof (// c\n T)` puts the leading run in the operand's head, so this
                // gate has to see through the shell or it disagrees with the builder about
                // whether the value breaks (the `=` would then break too, non-idempotently).
                //
                // Asked as the hang PREDICATE rather than through the hang seam, because
                // the seam now declines a shell the trailing-run rule RETAINS
                // (`paren_retains_for_trailing_run`) and would hand back the unwidened
                // start. The leading run is what makes the operand's head own the break;
                // whether the parens survive underneath it does not change that, and the
                // retained shell breaks internally either way. A shell with NO leading run
                // is left to the comment-driven arm below, where a trailing-`//`-only
                // operand is already pinned not to hug (`required_paren_shell_line_comment`
                // cases A / F).
                //
                // The window is the seam's, not the operand's own start, so a shell one
                // SUFFIX down (`keyof (// c⏎ T)[]`) is inside it too — that shell strips
                // into this same gap and hangs the operand, so reading the operand's start
                // here made the `=` break for a value that had already broken beneath it.
                let hang_start = self
                    .keyword_value_stripped_paren_hang(o.type_annotation)
                    .value_start;
                self.comments_force_own_line_between(kw_end, hang_start)
                    || self.stripped_paren_hang_has_leading_line_comment(o.type_annotation)
            }
            TSType::TypeQuery(q) => {
                let kw_end = q.span.start + "typeof".len() as u32;
                self.comments_force_own_line_between(kw_end, q.expr_name.span().start)
            }
            // The `[`→index frozen-route bracket layout (an alone-on-line directive
            // inside the brackets: `= A[⏎⇥// prettier-ignore⏎⇥K⏎]`) keeps the `[` on
            // the `=` line — the brackets own the break. Gated on `has_format_ignore`
            // so the directive-free common case pays no bracket scan.
            TSType::IndexedAccess(i) => {
                // The object's own required pair opening is the array twin's question one
                // construct over, so it is asked the same way — holding only
                // the frozen-bracket route below would have the two positions disagree.
                self.required_paren_pair_opens(
                    i.object_type,
                    type_needs_parens_for_indexed_access_object,
                ) || (self.has_format_ignore && {
                    let index_start = i.index_type.span().start;
                    self.find_char_outside_comments(i.object_type.span().end, index_start, b'[')
                        .is_some_and(|bp| self.member_gap_frozen(bp + 1, index_start))
                })
            }
            // The paren-interior frozen-route array element
            // (`= (⏎⇥// prettier-ignore⏎⇥T⏎)[]`) expands its own parens around the
            // run — the shell owns the break (`paren_interior_routed_inner` self-gates
            // on `has_format_ignore`). The required pair opening over a leading `//` or a
            // retained trailing run is the same fact, asked through the seam that decides
            // it (`required_paren_pair_opens`) rather than restated here.
            TSType::Array(a) => {
                self.paren_interior_routed_inner(a.element_type).is_some()
                    || self.required_paren_pair_opens(
                        a.element_type,
                        type_needs_parens_for_array_element,
                    )
            }
            // A redundant paren shell whose comments are ALL in its trailing gap
            // (`= (U // c)[]`, `= (A // c)`): with no separator following, the shell
            // RETAINS and opens over real hardlines — the value breaks internally, so
            // the `=` hugs it like a tuple or type literal rather than also breaking.
            TSType::Parenthesized(_) => self.paren_retains_for_trailing_run(ty),
            // A single-member union / intersection prints transparently as its member
            // (prettier drops the node in postprocess), so the `=` asks the member —
            // `= | (A // c)` collapses to the retained shell, which hugs.
            TSType::Union(u) if u.types.len() == 1 => {
                self.value_owns_its_comment_break(&u.types[0])
            }
            TSType::Intersection(x) if x.types.len() == 1 => {
                self.value_owns_its_comment_break(&x.types[0])
            }
            TSType::Literal(internal::TSLiteralType::TemplateLiteral(t)) => {
                self.template_literal_type_breaks_for_comment(t)
            }
            _ => false,
        }
    }

    fn build_type_alias_eq_value_doc(
        &self,
        decl: &internal::TSTypeAliasDeclaration<'_>,
        eq_pos: u32,
        has_complex_params: bool,
        lead_space: bool,
    ) -> DocId {
        let d = self.d();
        // A redundant paren shell around the RHS whose leading gap holds a line comment
        // (`type A = (/* b */ // c\n Z)`, and the double-nested form) strips to the same
        // fixed point the bare `type A = /* b */ // c\n Z` settles on. Widen the `=`→value
        // window to the unwrapped inner's start so the leading run (block + line) routes
        // through the force-break branch below, and build the value from the inner (plus
        // any trailing comment) — the shared keyword→value seam the other hang sites use.
        // When no line comment forces a hang, the seam returns the RHS unchanged, so the
        // block-comment / comment-free `=` layouts below are untouched.
        //
        // An alone-on-line format-ignore directive in the `=`→RHS gap freezes a
        // non-composite RHS verbatim (`single_child_frozen`; a union/intersection RHS
        // declines and freezes via its own leading-run walk). The frozen path keeps
        // the UNWIDENED window — an in-shell directive stays on the ordinary paths —
        // and the directive itself is emitted by the comment machinery below either
        // way (own-line comments already keep their own line here).
        let head = self.keyword_value_head(eq_pos + 1, &decl.type_annotation);
        // An alone-on-line directive INSIDE a redundant paren shell
        // (`type P = (⏎// prettier-ignore⏎{x:  1});`) hoists with the shell strip:
        // the widened window routes the whole leading run through the force-break
        // emission below (the directive keeps its own line) and the paren-stripped
        // INNER freezes, converging in ONE pass to the same fixed point the bare
        // authoring holds. Taken only when the inner actually freezes — a composite
        // inner keeps the ordinary strip-hang seam, where its own leading-run walk
        // (Rule A) reaches the same directive. It overrides the head seam's window,
        // the one head that widens past a shell under a freeze.
        let interior_frozen_inner = if head.frozen {
            None
        } else {
            self.paren_interior_routed_inner(&decl.type_annotation)
                .filter(|inner| is_freeze_target(inner))
        };
        let (type_start, value_type) = match interior_frozen_inner {
            Some(inner) => (inner.span().start, inner),
            None => (head.value_start, head.value_type),
        };
        let mut parts: DocBuf = smallvec![d.text(if lead_space { " =" } else { "=" })];

        // A leading comment between `=` and the RHS forces the value onto its own
        // line when it can't share the RHS's line: a line comment or multiline block
        // (`comments_force_own_line_between`), OR a single-line block comment sitting
        // *alone* on its own line (`type X =⏎/* c */⏎Y`). Prettier keeps such a
        // comment on its own line and renders the RHS below it. A single-line block
        // merely *glued to the RHS* across the `=`-break (`type X =⏎/* c */ Y`) is NOT
        // forced here — it leads the RHS and rides with it (the else branch's
        // `make_rhs`), so keying on the newline *before* the comment alone would
        // spuriously re-break already-broken output (`block_comment_isolated_own_line_between`
        // requires the newline *after* it too — the idempotency key).
        let force_break = self.comments_force_own_line_between(eq_pos + 1, type_start)
            || self.block_comment_isolated_own_line_between(eq_pos + 1, type_start);

        if force_break {
            // Line/multiline block comments force type to next line with indent.
            // Line comments stay on `=` line; multiline blocks go into the indent.
            // Example: `type A = // comment\n  B;`
            // Example: `type J =\n  /* comment\n   */\n  K | L;`
            let mut inline_parts = DocBuf::new();
            let mut indent_comment_parts = DocBuf::new();

            // Only the first single-line comment hugs the `=` line, and only when
            // it was *authored* on that line (`type A = /* c */ B`). An own-line
            // comment (`type A =⏎/* c */⏎B`) keeps its own line — prettier breaks
            // after `=` and never pulls it up. Multiline blocks (any position) and
            // every subsequent comment go on their own line in the indent. Two line
            // comments must not merge onto one line — the second `//` would stop
            // being a delimiter (a boundary loss).
            let comments: CommentVec<'_> = self
                .comments_to_emit_between(eq_pos + 1, type_start)
                .collect();
            for (idx, comment) in comments.iter().enumerate() {
                let multiline_block = comment.multiline;
                let authored_on_eq_line = self.is_same_line(eq_pos, comment.span.start);
                if idx == 0 && !multiline_block && authored_on_eq_line {
                    inline_parts.push(d.text(" "));
                    inline_parts.push(self.build_comment_doc(comment));
                } else {
                    indent_comment_parts.push(self.build_comment_doc(comment));
                    // Preserve an author blank line before the next comment, or before
                    // the value itself (`type X =⏎/* c */⏎⏎Y`), matching prettier.
                    //
                    // The separator is emitted AFTER each comment here, so the glue
                    // question is asked of the comment that FOLLOWS — the only safe
                    // spelling of it, and asked ONLY when a comment does follow: past the
                    // last one the `next` is the value, whose placement is the
                    // leading-side question this loop does not own.
                    let next_comment = comments.get(idx + 1);
                    let next = next_comment.map_or(type_start, |c| c.span.start);
                    if next_comment.is_some()
                        && self.trailing_run_hugs_previous(Some(comment), next)
                    {
                        // The author glued the pair onto one line — keep it.
                        indent_comment_parts.push(d.text(" "));
                    } else {
                        self.push_blank_preserving_hardline(
                            &mut indent_comment_parts,
                            comment.span.end,
                            next,
                        );
                    }
                }
            }

            parts.extend(inline_parts);

            // Type uses its own group (via build_type_doc) so unions/intersections
            // can independently decide whether to break. Built from the unwrapped inner
            // (equal to the RHS when no shell was stripped) plus any trailing comment
            // lifted from the shell; type position, so a trailing block trails the value
            // inline before the `;`. A frozen RHS is the verbatim
            // slice instead (redundant parens drop unless the shell holds a comment).
            let type_doc = if interior_frozen_inner.is_some() {
                // The frozen paren-stripped inner, with any trailing shell-gap
                // comment lifted after it so the strip stays lossless.
                self.with_stripped_paren_trailing(
                    self.build_frozen_single_child_doc(value_type),
                    &decl.type_annotation,
                    value_type,
                    TrailingBlock::Inline,
                )
            } else {
                self.build_keyword_value_doc(&head, TrailingBlock::Inline)
            };
            let mut indent_content: DocBuf = smallvec![d.hardline()];
            indent_content.extend(indent_comment_parts);
            indent_content.push(type_doc);
            parts.push(d.indent(d.concat(&indent_content)));
        } else {
            // A single-line block comment glued after `=` (`type A = /* c */ B`) leads
            // the RHS, so it rides *inside* the RHS doc rather than sitting on the `=`
            // head: on the `=` line while the RHS head hugs there, and down onto the
            // RHS's own line when the whole RHS relocates below `=`. `Trailing` spacing
            // (`/* c */ `) omits the leading space — every arm supplies its own (the
            // hug arms a literal `" "`, the hang/fluid arms their break `line`). Mirrors
            // the declarator's `make_init_doc` (`statements/variable.rs`); hoisting it
            // onto the `=` head instead would strand it there when
            // the RHS breaks below (a break-after-`=` union/reference/conditional).
            let lead_comment = self.build_comments_between_filtered_opt(
                eq_pos + 1,
                type_start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            );
            let make_rhs = |rhs: DocId| -> DocId {
                match lead_comment {
                    Some(comment_doc) => d.concat(&[comment_doc, rhs]),
                    None => rhs,
                }
            };
            // Every `fluid` marker in this function ties to the one assignment group.
            let fluid = |rhs: DocId| fluid_after_operator(d, rhs, GroupId::Assignment);

            // Check the type kind for different formatting rules. Redundant
            // comment-free parens around the RHS are stripped (prettier does the
            // same), so a `(union)` / `(intersection)` gets the same break layout
            // as the bare form instead of hanging inline. The doc is built from the
            // unwrapped type — safe, since we only unwrap when no comments are inside
            // the parens (commented parens stay on the preserve-in-place path).
            let value_type = self.unwrap_redundant_parens(&decl.type_annotation);
            // A frozen RHS never reaches this branch: an alone-on-line directive —
            // line or block spelling — always trips `force_break` over the same
            // window (`comments_force_own_line_between` catches every line comment;
            // `block_comment_isolated_own_line_between` is implied by the floor's
            // two-sided newline checks for a block one). The paren-interior freeze
            // widens the window to the inner's start, so its directive trips the
            // same gate.
            debug_assert!(
                !head.frozen && interior_frozen_inner.is_none(),
                "an alone-on-line directive always takes the force-break branch"
            );
            let build_value = || -> DocId { self.build_type_doc(value_type) };
            // A block run the author broke AFTER (`type A = /* c */⏎<value>`) takes
            // prettier's break-after-operator for EVERY value kind — `chooseLayout`'s
            // `hasLeadingOwnLineComment` arm wins ahead of the per-kind layouts, so it
            // is asked FIRST here: left to the arms below, the run glues onto a value
            // whose width-driven expansion then strands it mid-line (`⏎\t/* c */ | A`,
            // a form prettier never emits). A value that already carries a hard break
            // materializes the run's newline-after `line` as a blank-preserving
            // hardline; anything else rides one hang group with a soft `line`, which
            // breaks exactly when the `=` seam does — a value that FITS collapses to
            // the glued bytes in both formatters, so the flat render is unchanged.
            // (Own-line runs never reach this branch: `force_break` above owns them.)
            if let Some(run) = self.broke_after_value_leading_run(eq_pos + 1, type_start) {
                let type_doc = build_value();
                parts.push(self.break_or_hang_after_operator_run_doc(&run, type_start, type_doc));
            } else if let TSType::Union(u) = value_type {
                // A glued block run between `=` and a union with no authored leading
                // `|` is handed INTO the union, which binds it to the first member —
                // after the `| ` its broken layout synthesizes — instead of stranding
                // it ahead of the pipe (`/* c */ | A`, a form prettier never emits;
                // see `build_union_value_doc`). The run then must not also ride
                // `make_rhs` — exactly one of the two prints it.
                let (type_doc, run_handed) = self.build_union_value_doc(eq_pos + 1, u);
                let make_rhs =
                    |rhs: DocId| -> DocId { if run_handed { rhs } else { make_rhs(rhs) } };
                // `union_prints_hugged`, not the bare syntactic `should_hug_union_type`:
                // this must agree with the layout `build_union_type_doc` just chose. A
                // comment can make it decline the hug and expand, and then the `=` has
                // to break like any other non-hugging union.
                if self.union_prints_hugged(u) {
                    // Hugged unions (e.g., `{ ... } | null`): the object type handles its own
                    // expansion, so keep `= {` together like other internally-breaking types
                    parts.push(d.text(" "));
                    parts.push(make_rhs(type_doc));
                } else if u.types.len() == 1
                    && (self.value_owns_its_comment_break(&u.types[0])
                        || (matches!(unwrap_parenthesized(&u.types[0]), TSType::Intersection(_))
                            && d.will_break(type_doc)))
                {
                    // A single-member union prints transparently as its member (prettier
                    // drops the node in postprocess), so a member that owns its comment
                    // break hugs the `=` exactly as it would bare — `= | (A // c)`
                    // collapses to the retained shell, whose parens own the break. The
                    // hang below would split the `=` for a break the reparse (seeing the
                    // bare shell) reproduces at the shell, not the `=` (F1).
                    //
                    // A member that collapses to an INTERSECTION takes that arm's rule for
                    // the same reason, read off the doc the collapse just built: the
                    // intersection arm below hugs a `will_break` value and the bare
                    // authoring goes straight there, so hanging here split the `=` for a
                    // break the reparse — which no longer sees a union at all — reproduces
                    // inside the intersection (`= | ((// c⏎A | B) & C)`). The kind is asked
                    // because the break must be the MEMBER's: a member collapsing to a
                    // union breaks from the union's own leading-`|` layout, which is what
                    // the hang indents.
                    parts.push(d.text(" "));
                    parts.push(make_rhs(type_doc));
                } else if lead_space {
                    // Normal unions: break after `=` with leading `| ` and a hanging indent.
                    parts.push(hang_after_operator(d, make_rhs(type_doc)));
                } else {
                    // Pre-`=` comment continuation path (`type A<X> // c⏎= | a | b`):
                    // `lead_space` is false ONLY here, and the caller already wrapped the
                    // comment run + `=` + value in one `d.indent`
                    // (`build_continuation_indent`). A break must therefore NOT add the
                    // hang's extra indent — the members sit at the `=`'s level, not one
                    // deeper (else a double-indent). Still grouped so a short union stays
                    // inline on the `=` line (`= A | B`), matching the hang arm's flat
                    // case. A `_prettier_divergence` (type_alias_line_pre_equals_break):
                    // prettier relocates the comment after `=` and never emits this
                    // preserved-comment form.
                    parts.push(d.group(d.concat(&[d.line(), make_rhs(type_doc)])));
                }
            } else if let TSType::Intersection(i) = value_type {
                // Intersection types (prettier's `fluid`): the first member hugs the
                // `=` line when it fits and the intersection breaks after `=` when it
                // doesn't — continuation members then wrap with a hanging indent (the
                // boundary TypeLiteral/Mapped that owns its own expansion opts out; see
                // `intersection_hanging_with_indent`). The `fluid` marker's `line` is what
                // gives the LHS `<…>` group an early `fits()` exit at the `=`, so a
                // constrained-param header that fits stays inline instead of breaking the
                // type-param list when the first member overflows.
                //
                // A comment inside the intersection forces it to break; there the marker
                // must stay flat so the first member keeps hugging the `=` line (prettier's
                // behavior — `type B = a & // c⏎\tb`). The over-break this arm fixes is
                // always comment-free (a fit-driven break), so gate the marker on the
                // absence of a forced break and glue the comment case as before.
                let inter_doc = self.intersection_hanging_with_indent(i);
                if d.will_break(inter_doc) {
                    parts.push(d.text(" "));
                    parts.push(make_rhs(inter_doc));
                } else {
                    parts.push(fluid(make_rhs(inter_doc)));
                }
            } else if let TSType::Conditional(cond) = value_type {
                // Conditional types: break after `=` only if check/extends has type
                // parameters (prettier's `shouldBreakBeforeConditionalType` →
                // break-after-operator); otherwise `fluid`, which keeps the ternary on the
                // `=` line while its head fits and breaks after `=` once the head
                // overflows. The `fluid` marker keeps the LHS `<…>` inline (see the
                // intersection arm) rather than breaking the type-param list.
                let type_doc = build_value();
                if should_break_before_conditional_type(cond) {
                    parts.push(hang_after_operator(d, make_rhs(type_doc)));
                } else {
                    parts.push(fluid(make_rhs(type_doc)));
                }
            } else if type_has_internal_breaking(self, value_type) {
                // Types with internal breaking (braces, brackets, parens, angle brackets):
                // prettier's `fluid`. The marker hugs the `=` line when the value's first
                // break point is reachable within the width (`= {`, `= [`, `= Foo<`) — the
                // object/tuple/type-argument cases — and breaks after `=` when it is not
                // (a function type whose header pushes `(` past the width). Either way the
                // marker's `line` keeps the LHS `<…>` inline instead of breaking the
                // type-param list.
                let type_doc = build_value();
                parts.push(fluid(make_rhs(type_doc)));
            } else if has_complex_params {
                // Complex type parameters: use break-lhs layout
                // Type params break, `=` stays on same line, RHS stays inline
                // Example: type Foo<T extends string, U = number> = SomeLongType;
                // Breaks as:
                //   type Foo<
                //     T extends string,
                //     U = number,
                //   > = SomeLongType;
                let type_doc = build_value();
                parts.push(d.text(" "));
                parts.push(make_rhs(type_doc));
            } else {
                // Remaining types break after `=` with a hanging indent when too
                // long — unless a comment keeps the value on the `=` line, in which
                // case hug it there (like the internal-breaking / conditional arms):
                //
                //   - a prefix type operator (`keyof`/`typeof`) whose comment forces
                //     the operand onto its own line — the operator stays on `=`, its
                //     operand hangs via the operator's own layout; or
                //   - any value that `will_break` only from a *glued* comment (a
                //     multiline block whose operand stays inline — `keyof /* … */ B`,
                //     `A[/* … */ K]`): `hang_after_operator` would break on that
                //     spurious `will_break`, but prettier keeps it on the `=` line.
                //     The comment must not force an own-line break — a line comment
                //     or an own-line block hangs the operand for real and still
                //     breaks after `=` (e.g. indexed_access_line_comment).
                //
                // A comment-free value that doesn't `will_break` hangs (long case).
                let type_doc = build_value();
                let value_span = value_type.span();
                let value_has_comments =
                    self.has_comments_to_emit_between(value_span.start, value_span.end);
                // The `will_break` is what makes "bearing a comment" mean "laid out by
                // one": a value whose comments all ride `line_suffix` (a trailing run in
                // the index→`]` or stripped-paren gap) renders exactly as the comment-free
                // form, so it belongs on the `fluid` default below rather than on either
                // comment arm. Both arms ask this same question, so it is asked once —
                // they part only on whether the comment forces its own line.
                let comment_driven_break = value_has_comments && d.will_break(type_doc);
                let hug = self.value_owns_its_comment_break(value_type)
                    || (comment_driven_break
                        && !self.comments_force_own_line_between(value_span.start, value_span.end));
                if hug {
                    parts.push(d.text(" "));
                    parts.push(make_rhs(type_doc));
                } else if comment_driven_break
                    || matches!(
                        value_type,
                        TSType::Literal(internal::TSLiteralType::TemplateLiteral(_))
                    )
                {
                    // Break after `=` with a hanging indent, for two kinds:
                    //   - a non-hugging value whose comment ACTUALLY lays it out
                    //     (`comment_driven_break` above), preserving comment placement
                    //     (e.g. a `[`→index own-line comment hangs the index —
                    //     indexed_access_line_comment). Hanging a merely comment-BEARING
                    //     value wrapped it in a structural `indent` that a deferred run's
                    //     own break then inherited, printing the run one level too deep —
                    //     and that is NOT a fixed point, since the reparse reads the
                    //     comment at statement level (F1); and
                    //   - a template-literal type, whose `${…}` printer force-breaks: on
                    //     the `fluid` path the value would hug `= \`prefix_${` and break
                    //     the interpolation instead of breaking after `=` first (tsv's
                    //     template layout is already a deliberate divergence — see
                    //     template_literal_type_long; `is_simple_type_arg` excludes them
                    //     from atomic inlining for the same reason).
                    parts.push(hang_after_operator(d, make_rhs(type_doc)));
                } else {
                    // The comment-free remainder is prettier's `fluid` default
                    // (`chooseLayout`'s fallthrough): the value hugs the `=` line and
                    // breaks INSIDE its own delimiter when it can, and the marker's `line`
                    // keeps the LHS `<…>` inline instead of breaking the type-param list.
                    // A postfix wrapper (`(cond)[]`, `(cond)[K]`), a prefix operator
                    // (`keyof`/`readonly`/`typeof`) over a breakable operand, and an atomic
                    // reference all route here; an unbreakable value (a bare reference, a
                    // string-literal type) makes `fluid` and break-after-`=` render
                    // identically. `shouldBreakAfterOperator` has no case for these kinds,
                    // so prettier falls through to `fluid` — see
                    // `prettier/src/language-js/print/assignment.js`.
                    parts.push(fluid(make_rhs(type_doc)));
                }
            }
        }

        d.concat(&parts)
    }

    /// Build doc for interface declaration
    ///
    /// Uses group mode when extends has multiple items - heritage breaks when group breaks.
    pub(super) fn build_interface_declaration_doc(
        &self,
        decl: &internal::TSInterfaceDeclaration<'_>,
    ) -> DocId {
        let d = self.d();

        // Compute positions for heritage comment extraction
        let pre_heritage_end = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.end);
        // Use `extends` keyword position (not first heritage item start) so
        // heritage leading comments only cover name-to-extends, not extends-to-item
        let extends_keyword_start = decl
            .extends
            .first()
            .and_then(|e| self.find_keyword_in_range(pre_heritage_end, e.span.start, "extends"));
        let first_extends_start =
            extends_keyword_start.or_else(|| decl.extends.first().map(|e| e.span.start));

        // Comments between name/type-params and extends force group mode
        let has_heritage_comments = first_extends_start.is_some_and(|ext_start| {
            self.has_comments_to_emit_between(pre_heritage_end, ext_start)
        });
        let has_heritage_line_comments = first_extends_start
            .is_some_and(|ext_start| self.has_line_comments_between(pre_heritage_end, ext_start));

        // Group mode: multiple extends items OR heritage comments
        let group_mode = decl.extends.len() > 1 || has_heritage_comments;

        let mut header_parts: DocBuf = smallvec![];
        // Word by word, so `declare`'s own gap before `interface` keeps its comment
        // instead of folding into the keyword→name scan below.
        let (keyword_doc, keyword_end) = self.build_declaration_head_doc(
            decl.declare,
            &["interface"],
            decl.span.start,
            decl.id.span.start,
        );
        header_parts.push(keyword_doc);
        // Comments between keyword and name: `interface /* c */ A {}`
        header_parts.push(d.text(" "));
        if let Some(comments) = self
            .build_inline_comments_between_doc_trailing_space_opt(keyword_end, decl.id.span.start)
        {
            header_parts.push(comments);
        }
        header_parts.push(self.identifier_name_doc(&decl.id));

        // Comments between name and type params: `interface A/* c */ <T> {}`
        // Line comments get a hardline to prevent absorbing type params as comment text
        if let Some(type_params) = &decl.type_parameters {
            self.push_name_to_type_params_comments(
                &mut header_parts,
                decl.id.span.end,
                type_params.span.start,
                CommentSpacing::Trailing,
            );
        }

        // Build extends doc, with comments between `extends` keyword and first item
        let extends_doc = if !decl.extends.is_empty() {
            Some(self.build_heritage_clause_doc(
                HeritageKeyword::Extends,
                decl.extends,
                group_mode,
                extends_keyword_start,
            ))
        } else {
            None
        };

        // Build the header group (without body - body has hardlines that would force breaking)
        let header_doc = if group_mode {
            // Group mode: one unified group - when it breaks, extends breaks too
            if let Some(type_params) = &decl.type_parameters {
                // Type params get their own group - break independently of extends
                header_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }

            // Comments between name/type-params and extends
            if let Some(ext_start) = first_extends_start {
                let (inline, indent) =
                    self.build_heritage_leading_comment_parts(pre_heritage_end, ext_start);
                header_parts.extend(inline);

                // Extends clause with line break, preceded by any extra heritage comments
                if let Some(ext_doc) = extends_doc {
                    let mut heritage_parts = indent;
                    heritage_parts.push(d.line());
                    heritage_parts.push(ext_doc);
                    header_parts.push(d.indent(d.concat(&heritage_parts)));
                }
            } else if let Some(ext_doc) = extends_doc {
                header_parts.push(d.indent(d.concat(&[d.line(), ext_doc])));
            }

            let parts_doc = d.concat(&header_parts);
            if has_heritage_line_comments {
                d.group_break(parts_doc)
            } else {
                d.group(parts_doc)
            }
        } else {
            // Non-group mode: type params break independently, extends stays inline
            // (No heritage comments in this path - comments force group mode)
            if let Some(type_params) = &decl.type_parameters {
                header_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }

            // Extends clause stays inline
            if let Some(ext_doc) = extends_doc {
                header_parts.push(d.text(" "));
                header_parts.push(ext_doc);
            }

            d.concat(&header_parts)
        };

        // Handle comments between header and body: interface B /* comment */ {
        let header_end = if let Some(last_ext) = decl.extends.last() {
            last_ext.span.end
        } else if let Some(tp) = &decl.type_parameters {
            tp.span.end
        } else {
            decl.id.span.end
        };
        // Comments between the header and body `{`, plus the pre-brace spacing, plus the
        // body's format-ignore verdict. Shared with the class printer: each comment is
        // kept on its own line (a line comment doesn't absorb a following one), and a
        // line comment forces the brace onto the next line. See
        // heritage_last_item_line_comment.
        let (pre_body, frozen_body) =
            self.build_declaration_pre_body_doc(header_end, decl.body.span);
        let mut parts: DocBuf = smallvec![header_doc, pre_body];

        if let Some(frozen) = frozen_body {
            parts.push(self.build_frozen_span_doc(frozen));
        } else if decl.body.body.is_empty() {
            parts.push(self.build_empty_body_with_comments_doc(decl.body.span));
        } else {
            // A comment trailing the opening `{` on its own line is kept on the
            // `{` line when the body expands (divergence from prettier, which
            // relocates it to its own line as the first member's leading
            // comment). See conformance_prettier_ts_comments.md §Comment relocation
            // (Class/interface/enum body `{`).
            let first_member_start = decl.body.body[0].span().start;
            let (brace_line_prefix, delimiter_pull_pos) =
                self.delimiter_line_comment_prefix(decl.body.span.start, first_member_start);
            parts.push(d.text("{"));
            if let Some(prefix) = brace_line_prefix {
                parts.push(prefix);
            }
            parts.push(d.indent(d.concat(&[self.build_type_elements_doc(
                decl.body.body,
                decl.body.span.start,
                decl.body.span.end,
                delimiter_pull_pos,
            )])));
            parts.push(d.hardline());
            parts.push(d.text("}"));
        }

        d.concat(&parts)
    }

    /// Build doc for declare function with wrapping support for type parameters
    pub(super) fn build_declare_function_doc(
        &self,
        decl: &internal::TSDeclareFunction<'_>,
        clause_tail: Option<u8>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        // The opening modifier (`async`, or the `declare` a top-level ambient function
        // carries — implicit, and so absent, inside a `declare namespace`), the gap before
        // `function`, the keyword and a generator `*`: the same head the declaration and
        // the expression print. The third site of one gap — pushing a bare
        // `async ` here and letting the comment fall through to the `function`→name emitter
        // would relocate it across the keyword (`async /* c */ function f(): T;` →
        // `async function /* c */ f(): T;`).
        let head_end = self.push_function_keyword_head(
            &mut parts,
            decl.span.start,
            decl.id.span.start,
            FunctionHeadModifier::from_flags(decl.r#async, decl.declare),
            decl.generator,
        );

        // Find paren position for comment boundary and later comment handling
        let paren_search_start = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.end);
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        // Everything after the `function`→name gap is collected into `tail` (the
        // continuation), so a *line* comment in that gap indents the whole
        // signature one level (uniform declaration-header rule).
        let mut tail = smallvec![self.identifier_name_doc(&decl.id)];

        // Comments between name and type params/parens: `declare function fn1/* c */ <T>()` or `fn1 /* c */()`
        // Line comments get a hardline to prevent absorbing type params as comment text
        self.push_signature_head_comments(
            &mut tail,
            decl.id.span.end,
            decl.type_parameters.as_ref(),
            paren_pos.unwrap_or(decl.id.span.end),
        );

        // Type parameters with wrapping support
        if let Some(type_params) = &decl.type_parameters {
            tail.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Params + return in one signature group (preserves a comment between `)`
        // and `:`), so a too-long signature breaks the params before the return-type
        // generic — and a long type-param list breaking above doesn't drag the params
        // open (they stay inline when `>(a, b): R` fits).
        let sig_doc = self.build_signature_params_return_group(
            decl.params,
            decl.type_parameters.as_ref(),
            decl.return_type.as_ref(),
            paren_pos,
        );
        // Comments between type_params and `(` go after type_params
        let gap = decl
            .type_parameters
            .as_ref()
            .map(|t| t.span.end)
            .zip(paren_pos);
        self.append_signature_head_gap_comments(&mut tail, gap, None, sig_doc);

        // Comments between return type (or `)`) and `;`. An own-line comment defers
        // past the `;` (prettier); here the `;` is in this same doc, so emit it locally.
        let mut deferred = DocBuf::new();
        self.append_signature_end_comments(
            &mut tail,
            decl.return_type.as_ref(),
            paren_pos,
            decl.span.end,
            &mut deferred,
            clause_tail,
        );

        tail.push(d.text(";"));
        tail.extend(deferred);

        // Comments between `function` keyword and name; a line comment indents the
        // whole continuation (uniform declaration-header rule). From `head_end`, not the
        // span start: the head already printed the `async`→`function` gap, and re-reading
        // it here printed those comments twice.
        parts.push(self.build_keyword_to_name_continuation(
            head_end,
            decl.id.span.start,
            d.concat(&tail),
        ));

        d.group(d.concat(&parts))
    }

    /// Build doc for entity name
    pub(crate) fn build_entity_name_doc(&self, name: &internal::TSEntityName<'_>) -> DocId {
        // Delegate to standalone function - doesn't need printer state
        build_entity_name_doc(self, name)
    }

    /// Build doc for type elements with comment handling
    ///
    /// `delimiter_pull_pos`, when `Some(pos)`, drops the first member's leading
    /// comments that share a source line with `pos` (the opening `{`) — the
    /// caller emits those as a prefix on the `{` line instead (the open-brace
    /// trailing-comment divergence). Pass `None` to keep the default behavior.
    fn build_type_elements_doc(
        &self,
        members: &[internal::TSTypeElement<'_>],
        body_start: u32,
        body_end: u32,
        delimiter_pull_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();

        // The shared member-body walk — the same one the class body and both
        // type-literal force-multiline walks take. An interface body's own facts: a
        // member's span already covers its separator (so the span end IS this family's
        // gap floor), the member doc prints its own `;`, and a directive freezes the
        // whole member span.
        //
        // Zero-comment fast gate: one binary search over the whole body span
        // short-circuits every per-member comment sub-query (leading collect,
        // format-ignore lookup, trailing-comment scan, trailing-body comments). Sound
        // because comments are disjoint + start-sorted and every sub-range lies within
        // `[body_start, body_end)`. Blank-line preservation is comment-independent and
        // stays outside the gate.
        self.build_member_list_docs_into(
            &mut parts,
            members,
            MemberBody {
                span: Span::new(body_start, body_end),
                has_comments: self.has_comments_on_page_between(body_start, body_end),
                delimiter_pull_pos,
                blank_scan: MemberBlankScan::PastComments,
                freeze: MemberFreeze::Span,
                seam: MemberSeam::Whole {
                    floor: MemberFloor::MemberEnd,
                },
            },
            internal::TSTypeElement::span,
            |member| member.span().end,
            |member, _deferred| self.build_type_element_doc(member),
        );

        d.concat(&parts)
    }

    /// Build doc for a single type element
    fn build_type_element_doc(&self, elem: &internal::TSTypeElement<'_>) -> DocId {
        // Every member doc is shared with the type-literal printer (via the same
        // dispatcher); the only difference is the interface member carries its own
        // `;`, with any own-line comment in the member→`;` gap deferred past it.
        let mut deferred = DocBuf::new();
        let member = self.build_type_member_doc_inner(elem, &mut deferred);
        self.build_member_with_semicolon_doc(member, deferred)
    }

    /// Print an enum declaration: `enum Foo { A, B }` or `const enum Foo { A = 1 }`
    ///
    /// Build doc for enum declaration
    ///
    /// Prettier format:
    /// ```text
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    /// ```
    pub(super) fn build_enum_declaration_doc(
        &self,
        decl: &internal::TSEnumDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        let mut prefix = DocBuf::new();

        // `declare` / `const` prefixes, word by word: this is the only three-word
        // declaration head (`declare const enum`), so it is where folding the run into
        // one measured span costs the most — both interior gaps land on the name at once.
        let head: &[&'static str] = if decl.r#const {
            &["const", "enum"]
        } else {
            &["enum"]
        };
        let (keyword_doc, keyword_end) = self.build_declaration_head_doc(
            decl.declare,
            head,
            decl.span.start,
            decl.id.span.start,
        );
        prefix.push(keyword_doc);

        // Everything after the `enum`→name gap is collected into `parts` (the
        // continuation), so a *line* comment in that gap indents the whole
        // declaration one level (uniform declaration-header rule).
        let mut parts: DocBuf = smallvec![self.identifier_name_doc(&decl.id)];

        // Handle comments between name and body: enum C /* comment */ {
        // Use comment-aware search to skip `{` inside comments.
        let enum_body_brace =
            self.find_char_outside_comments(decl.id.span.end, decl.span.end, b'{');

        // Find body start (after '{')
        let body_start = enum_body_brace.map_or(decl.span.start, |b| b + 1);
        let body_end = decl.span.end.saturating_sub(1); // Before '}'
        let body_span = Span::new(body_start - 1, decl.span.end); // Include '{' and '}'

        // The header→`{` gap: its comments plus the pre-`{` spacing, and the body's
        // format-ignore verdict. A line comment drops the brace to the next line —
        // emitting the gap inline and appending a bare `" "` let the `//` swallow it
        // (`enum E // c {`), output that does not reparse.
        let (pre_body, frozen_body) =
            self.build_declaration_pre_body_doc(decl.id.span.end, body_span);
        parts.push(pre_body);

        if let Some(frozen) = frozen_body {
            parts.push(self.build_frozen_span_doc(frozen));
        } else if decl.members.is_empty() {
            // Empty enum body - handle comments inside (a fitting block comment
            // stays inline as `enum E {/* c */}`).
            parts.push(self.build_empty_braces_inline_with_comments_doc(body_span));
        } else {
            // A comment trailing the opening `{` on its own line is kept on the
            // `{` line when the body expands (divergence from prettier, which
            // relocates it to its own line as the first member's leading
            // comment). See conformance_prettier_ts_comments.md §Comment relocation
            // (Class/interface/enum body `{`). `body_start - 1` is the `{`.
            let first_member_start = decl.members[0].span.start;
            let (brace_line_prefix, delimiter_pull_pos) =
                self.delimiter_line_comment_prefix(body_start - 1, first_member_start);

            parts.push(d.text("{"));
            if let Some(prefix) = brace_line_prefix {
                parts.push(prefix);
            }
            // Build member docs with comment handling
            let mut member_parts = DocBuf::new();
            // Where the next member's LEADING scan resumes: the end of the previous
            // member's trailing run, NOT past its comma. A comment the author wrote before
            // a comma they pushed onto its own line (`A⏎// c1⏎, B`) is claimed by no
            // trailing run — they take the member's own line only — so a scan starting
            // past the comma left it with no emitter at all, a DROPPED comment. The run's
            // claim is a prefix of the gap, so resuming at its end also can't re-print it.
            let mut prev_end = body_start;

            // Zero-comment fast gate: one binary search over the whole
            // enum-body span short-circuits every per-member comment sub-query
            // (leading collect, format-ignore lookup, and the trailing-body
            // comments). Sound because comments are disjoint + start-sorted and
            // every sub-range lies within `body_span`, so when none sit inside
            // the body all sub-queries are provably empty/false. The trailing run
            // carries its own zero-comment gate (`collect_trailing_comments`), so
            // it needs no arm here.
            let body_has_comments =
                self.has_comments_to_emit_between(body_span.start, body_span.end);

            for (i, member) in decl.members.iter().enumerate() {
                let member_start = member.span.start;
                let is_first = i == 0;
                let is_last = i == decl.members.len() - 1;

                // The rest of the gap since the previous member's trailing run — the
                // element-comma partition (see `collect_item_leading_comments`).
                let comments: CommentVec<'_> = if body_has_comments {
                    self.collect_item_leading_comments(
                        prev_end,
                        member_start,
                        is_first.then_some(delimiter_pull_pos).flatten(),
                    )
                } else {
                    CommentVec::new()
                };

                // Check for blank lines
                if !is_first {
                    self.push_item_blank_separator(&mut member_parts, prev_end, member_start);
                }

                // Process leading comments
                self.push_leading_comment_run(
                    &mut member_parts,
                    comments.iter().copied(),
                    member_start,
                    LeadingGlue::Adjacent,
                );

                // A preceding format-ignore directive keeps the member's source
                // verbatim. The member span excludes the trailing `,`, which the loop
                // still appends below. Resolved once as the SLICE rather than a bool,
                // because the trailing seam below needs the same answer — one gap
                // resolution, one source of truth.
                let frozen_span = (body_has_comments
                    && self.member_gap_frozen(prev_end, member_start))
                .then_some(member.span);
                member_parts.push(match frozen_span {
                    Some(slice) => self.raw_source_doc(slice),
                    None => self.build_enum_member_doc(member),
                });

                // Where the trailing seam starts: the member's PRINTED end, not
                // `span.end`. The span runs through a grouping paren the initializer
                // stops inside (`A = (a, b)`), and the paren this loop prints is the
                // printer's own — so a span anchor starts past the shell's interior,
                // which no other emitter scans (a DROPPED comment). Under a freeze the
                // verbatim slice already printed that interior, so the anchor moves to
                // its end instead, or the seam prints the comment a second time.
                // `docs/comments.md` §The element-comma seam.
                let member_end = Self::element_claim_anchor(frozen_span, member.printed_end());
                let upper_bound = decl
                    .members
                    .get(i + 1)
                    .map_or(body_end, |next| next.span.start);

                // The trailing run around the separator, on the shared element-comma
                // contract (`collect_trailing_comments` / `push_element_comma_trailing`) —
                // the same one the object-literal, destructuring-pattern and specifier
                // loops use. Blocks keep their side of the comma, line comments defer via
                // `line_suffix`, and the claim is a prefix of the gap, so this member's
                // run and the next member's leading scan partition it. No trailing comma
                // on the last member under `trailingComma: 'none'`.
                let trailing = self.collect_trailing_comments(member_end, upper_bound, is_last);
                let comma = (!is_last).then(|| d.text(","));
                self.push_element_comma_trailing(&mut member_parts, &trailing, comma);
                prev_end = trailing.end_pos;
            }

            // Handle trailing comments after the last member
            if body_has_comments {
                self.push_trailing_body_comments(&mut member_parts, prev_end, body_end, false);
            }

            parts.push(d.indent_hardline(d.concat(&member_parts)));
            parts.push(d.hardline());
            parts.push(d.text("}"));
        }

        // Comments between `enum` and the name; a line comment indents the whole
        // continuation (uniform declaration-header rule). From `keyword_end`, not the
        // span start: the head above already emitted the `declare`→`const`→`enum`
        // gaps, and scanning from the start here would print them a second time — and
        // at the wrong position.
        prefix.push(self.build_keyword_to_name_continuation(
            keyword_end,
            decl.id.span.start,
            d.concat(&parts),
        ));
        d.concat(&prefix)
    }

    /// Build doc for a single enum member
    fn build_enum_member_doc(&self, member: &internal::TSEnumMember<'_>) -> DocId {
        let d = self.d();
        // Member id (identifier or string literal)
        let id_doc = match &member.id {
            internal::TSEnumMemberId::Identifier(id) => self.identifier_name_doc(id),
            internal::TSEnumMemberId::String(lit) => {
                // String literal member name: `"hello"` in `enum { "hello" = 1 }`
                self.build_literal_doc(lit)
            }
        };

        // Initializer: ` = value`
        if let Some(init) = &member.initializer {
            // Extract comments between `=` and initializer value
            let id_end = member.id.span().end;
            let init_start = init.span().start;
            let eq_pos = self.find_equals_position(id_end, init_start);

            // An enum member's `=` is a value gap (`mark_jsdoc_cast_value_gap`).
            self.mark_jsdoc_cast_value_gap(init);

            // The `=`→value head: an own-line directive in the gap freezes the whole value,
            // exactly as at the declarator initializer and every other host of the
            // assignment family. Through [`Printer::build_value_head_doc`] because this
            // position's ordinary emission IS the plain expression doc — the member loop's
            // own trailing scan claims the value→`,` gap
            // (`Printer::collect_trailing_comments`, anchored past the frozen slice), so
            // there is no paren shell here for the freeze to ride inside.
            let init_doc = self.build_value_head_doc(eq_pos + 1, init);

            // The post-`=` value content (shared by the inline and the
            // continuation forms). For binary expressions, indent so wrapped
            // continuations align under the value; any `=`→value block comment
            // leads it. A frozen value is unaffected either way — a verbatim slice
            // holds no `line` for the indent to act on.
            let init_with_indent = if matches!(init, internal::Expression::BinaryExpression(_)) {
                d.indent(init_doc)
            } else {
                init_doc
            };
            let value_doc = self.prepend_rhs_comments(init_with_indent, eq_pos + 1, init_start);

            // A line comment between the name and `=` keeps the comment after the
            // name and drops `= value` to a continuation line indented one level
            // (preserve position — lossless when a second comment also trails the
            // member; prettier relocates past the value and merges the two onto one
            // line — see conformance_prettier_ts_comments.md §Comment relocation).
            if let Some(cont) = self.build_initializer_line_continuation(
                id_end,
                eq_pos,
                // A type alias's RHS is a TYPE — it owns no comment, so the gap's bound is
                // the whole of what this seam needs from it.
                ContinuationValue::Opaque(init_start),
                || value_doc,
            ) {
                d.concat(&[id_doc, cont])
            } else {
                // Comments between name and `=` (block stays inline: `a /* c */ = 1`)
                let id_doc = if self.has_comments_to_emit_between(id_end, eq_pos) {
                    d.concat(&[
                        id_doc,
                        self.build_inline_comments_between_doc(id_end, eq_pos),
                    ])
                } else {
                    id_doc
                };
                // The `=`→value gap shares the initializer comment layout with variable
                // declarators and for-loop init clauses. A **line** comment partitions
                // (trailing on the `=` line, the rest leading the value); an **own-line
                // or multiline block** hangs after the operator, keeping the comment on
                // its own line; a **glued single-line block** returns `None` and falls
                // through to the inline `= /* c */ value` form below.
                //
                // Sharing the helper is what makes this gap agree with every other
                // initializer. Emitting the run positionally instead (`prepend_rhs_comments`
                // alone) both preserved a break the siblings reflow AND relocated an
                // own-line comment up onto the `=` line — and that relocation is not
                // idempotent, since the moved comment reads as glued on the next pass and
                // then collapses. The helper cannot drift that way: it decides the layout
                // from the comment's authored position and emits the matching shape.
                if let Some(rhs) =
                    self.build_eq_comment_break_rhs(eq_pos, init_start, " =", || init_with_indent)
                {
                    d.concat(&[id_doc, rhs])
                } else if self.is_own_line_jsdoc_cast(init) {
                    // The helper above reads the `=`→value gap, which an OWNED comment never
                    // reaches — it is glued to the value's first token and rides inside its
                    // doc (`docs/comments.md` hazard 2). An own-line cast's comment still
                    // decides this layout: the cast prints a hardline between it and its `(`,
                    // so without the matching hang the `(` lands at the member's own indent
                    // and the next pass collapses it — an authoring with no fixed point. Same
                    // narrow test, same reason, as the binding defaults; the wider
                    // `owned_leading_comment_effect` would also hang an indentable block this
                    // member keeps inline (`member_init_multiline_block_comment`).
                    d.concat(&[id_doc, d.text(" ="), hang_after_operator(d, value_doc)])
                } else {
                    // Only a glued single-line block (or no comment) reaches here — the
                    // helper claimed every authoring that hangs. The value gets its own
                    // `group` so the leading run's soft `line` is measured against the
                    // value rather than the enum body, which is hardline-joined and so
                    // always broken; without it the `line` rendered as a newline and
                    // preserved a break every sibling initializer reflows. This is the
                    // array-family/params-family distinction: an element in its own group
                    // collapses, an unwrapped one inherits the broken parent. Safe only
                    // because the own-line authoring no longer passes through here — when
                    // it did, this group made the format non-idempotent.
                    d.concat(&[id_doc, d.text(" = "), d.group(value_doc)])
                }
            }
        } else {
            id_doc
        }
    }

    /// Build doc for namespace/module declaration
    ///
    /// Prettier format:
    /// ```text
    /// namespace Utils {
    ///     export function log() {}
    /// }
    /// ```
    pub(super) fn build_module_declaration_doc(
        &self,
        decl: &internal::TSModuleDeclaration<'_>,
        clause_tail: Option<u8>,
    ) -> DocId {
        self.build_module_declaration_doc_inner(decl, true, clause_tail)
    }

    /// Inner helper for module declaration doc building
    /// `is_root` is true for the outermost declaration (prints `namespace` keyword)
    fn build_module_declaration_doc_inner(
        &self,
        decl: &internal::TSModuleDeclaration<'_>,
        is_root: bool,
        clause_tail: Option<u8>,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();

        // Only print keywords for root declaration — a nested one (`namespace A.B {}`'s
        // `B`) is reached through the dotted-pair printer with its head already emitted.
        if is_root {
            // `global` is special — it replaces the namespace/module keyword and is the
            // name as well, so its head is `declare global` with nothing after it. That
            // makes it the one arm with no keyword→name gap: a comment in the
            // `declare`→`global` gap has no later name to be relocated onto, and stays
            // before `global` in prettier too. The shared head builder emits it there,
            // and a bare `global {}` reduces to the single-word case (the span starts at
            // `global`, so the window is empty).
            if decl.global {
                let (keyword_doc, _) = self.build_declaration_head_doc(
                    decl.declare,
                    &["global"],
                    decl.span.start,
                    decl.id.span().end,
                );
                parts.push(keyword_doc);
            } else {
                // Word by word, so `declare`'s gap before the head keyword keeps its
                // comment rather than folding into the keyword→name scan that follows.
                let head: &[&'static str] = match decl.kind {
                    internal::TSModuleDeclarationKind::Namespace => &["namespace"],
                    internal::TSModuleDeclarationKind::Module => &["module"],
                };
                let name_start = decl.id.span().start;
                let (keyword_doc, keyword_end) = self.build_declaration_head_doc(
                    decl.declare,
                    head,
                    decl.span.start,
                    name_start,
                );
                parts.push(keyword_doc);
                parts.push(d.text(" "));
                // Comments between the head keyword and the name:
                // `declare namespace /* c */ A {}`. Emitted here rather than beside the
                // name below so `keyword_end` never has to outlive the keyword that
                // defines it — a nested declaration prints no keyword and opens no such
                // gap, which is why only this arm asks.
                if let Some(comments) = self
                    .build_inline_comments_between_doc_trailing_space_opt(keyword_end, name_start)
                {
                    parts.push(comments);
                }
            }
        }

        // Module/namespace name (if not global)
        if !decl.global {
            let name_doc = match &decl.id {
                internal::TSModuleName::Identifier(id) => self.identifier_name_doc(id),
                internal::TSModuleName::Literal(lit) => self.build_literal_doc(lit),
            };
            // A dotted namespace (`namespace Outer.Inner {}`) pairs this name with the
            // nested one, and both gaps around that `.` are positions an author can
            // comment in — so the shared dotted-pair printer emits the name, the dot and
            // the gaps together (it needs the left doc, hence holding it until here).
            // Pushing name + `d.text(".")` scans neither gap and drops what's in it.
            match &decl.body {
                Some(internal::TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                    let name_end = decl.id.span().end;
                    parts.push(self.build_dotted_pair_doc(
                        name_doc,
                        self.build_module_declaration_doc_inner(nested, false, None),
                        name_end,
                        nested.span.start,
                    ));
                }
                _ => parts.push(name_doc),
            }
        }

        // Body (may be None for shorthand: `declare module 'name';`)
        match &decl.body {
            Some(internal::TSModuleDeclarationBody::TSModuleBlock(block)) => {
                // Handle comments between name and body: namespace D /* comment */ {
                // `decl.id` is the name, or `global` where that keyword is both.
                let name_end = decl.id.span().end;

                // The header→`{` gap: its comments plus the pre-`{` spacing, and the
                // body's format-ignore verdict. A line comment drops the brace to the
                // next line — emitting the gap inline and appending a bare `" "` let the
                // `//` swallow it (`namespace D // c {`), output that does not reparse.
                let (pre_body, frozen_body) =
                    self.build_declaration_pre_body_doc(name_end, block.span);
                parts.push(pre_body);

                // Comments attached to a body whose only statements are
                // dropped `EmptyStatement`s are still picked up by
                // `build_empty_body_with_comments_doc`, which scans the full
                // brace range rather than the statement list.
                if let Some(frozen) = frozen_body {
                    parts.push(self.build_frozen_span_doc(frozen));
                } else if is_effectively_empty_body(block.body) {
                    // Empty namespace body - handle comments inside
                    parts.push(self.build_empty_body_with_comments_doc(block.span));
                } else {
                    // A comment trailing the opening `{` on its own line is kept on
                    // the `{` line when the body expands (divergence from prettier,
                    // which relocates it to its own line as the body's leading
                    // comment). Same mechanism as block-statement bodies. See
                    // conformance_prettier_ts_comments.md §Comment relocation (Namespace/module
                    // body `{`).
                    let first_stmt_start = block.body[0].span().start;
                    let (brace_line_prefix, delimiter_pull_pos) =
                        self.delimiter_line_comment_prefix(block.span.start, first_stmt_start);

                    parts.push(d.text("{"));
                    if let Some(prefix) = brace_line_prefix {
                        parts.push(prefix);
                    }

                    // Shared per-statement walk (leading comments, blank-line
                    // separators, format-ignore, trailing same-line comments) —
                    // same as block-statement bodies. A `TSModuleBlock` isn't a
                    // Program/BlockStatement, so its bare string statements are
                    // never directive-prologue eligible — see
                    // `Printer::needs_avoid_directive_parens`.
                    let body_start = block.span.start + 1; // After opening '{'
                    let body_end = block.span.end.saturating_sub(1); // Before '}'
                    let mut stmt_parts = d.pooled_docbuf();
                    let tail = self.build_statement_list_docs_into(
                        &mut stmt_parts,
                        block.body,
                        Span::new(body_start, body_end),
                        false,
                        delimiter_pull_pos,
                        false,
                    );

                    // Handle own-line trailing comments after the last statement
                    self.push_trailing_body_comments(
                        &mut stmt_parts,
                        tail.prev_end,
                        body_end,
                        tail.claims_trailing,
                    );

                    parts.push(d.indent_hardline(d.concat(&stmt_parts)));
                    parts.push(d.hardline());
                    parts.push(d.text("}"));
                }
            }
            Some(internal::TSModuleDeclarationBody::TSModuleDeclaration(_)) => {
                // A dotted namespace (`namespace Outer.Inner {}`) is emitted with the
                // name above — the `.` and both its gaps belong to that pair, and only
                // that path holds the left doc the shared printer needs. A `global`
                // namespace can't be dotted, so the name path always ran.
            }
            None => {
                // Shorthand ambient module: `declare module 'name';`.
                //
                // The name→`;` gap is inside the node span, so no enclosing emitter
                // reaches it — without this scan every comment there is DROPPED
                // (docs/comments.md hazard 4). `block_after_separator` is `false`
                // because prettier attaches the comment to the *name* and appends the
                // `;` after it, so a same-line block stays before the terminator
                // (`declare module 'a' /* c */;`) — the `import =` / `export =` side
                // of that axis, not the statement-`;` side a `const` takes.
                //
                // Where the shorthand ended by ASI the span stops at the name and the
                // gap is empty; any comment past it belongs to the statement's own
                // trailing emitter.
                self.push_semicolon_with_gap_comments(
                    &mut parts,
                    decl.id.span().end,
                    decl.span.end,
                    false,
                    clause_tail,
                );
            }
        }

        d.concat(&parts)
    }
}
