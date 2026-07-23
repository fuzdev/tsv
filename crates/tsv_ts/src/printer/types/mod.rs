// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Complex types (unions, intersections, generics, etc.)
//
// This module coordinates type printing and delegates to specialized submodules:
// - helpers.rs: Standalone helper functions (parenthesization, unwrapping)
// - type_params.rs: Type parameter declarations and instantiation
// - type_annotation.rs: Type annotations (`: Type`)
// - type_arguments.rs: Type-argument instantiation (`<T, U>`) rendering
// - type_members.rs: Type literal members (PropertySignature, MethodSignature, etc.)
// - type_literal.rs: Type literals (`{ a: T }`) and object alignment
// - function_types.rs: Function types, constructor types, signature params
// - union_intersection.rs: Union and intersection types
// - composite.rs: Conditional, mapped, tuple, array types
// - literal_types.rs: Literal types (string, number, template literal)

mod composite;
pub(in crate::printer) mod function_types;
pub(crate) mod helpers;
mod literal_types;
mod type_annotation;
mod type_arguments;
mod type_literal;
mod type_members;
mod type_params;
mod union_intersection;

// Re-export public items from helpers
pub use helpers::unwrap_parenthesized;

// Re-export for submodules to use `super::X` instead of `super::super::X`
pub(super) use super::comments::BlankRule;
pub(super) use super::{CommentFilter, CommentSpacing, Printer};

use crate::ast::internal::{TSImportType, TSParenthesizedType, TSType};
use crate::printer::CommentVec;
use crate::printer::calls::PartitionedComments;
use crate::printer::layout::hang_after_operator;
use helpers::type_needs_parens_for_indexed_access_object;
use helpers::type_needs_parens_for_optional_element;
use helpers::type_needs_parens_for_prefix_operator;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    //
    // Main Type Doc Builders
    //

    /// Build a Doc for a TypeScript type expression.
    ///
    /// A `TypeReference`'s own type arguments always break internally when too wide
    /// (`Promise<LongType | null>` breaks inside the `<>`) — `build_type_arguments_doc` is the
    /// single builder for every type-argument position, so no caller has to opt into that.
    pub(in crate::printer) fn build_type_doc(&self, ts_type: &TSType<'_>) -> DocId {
        let d = self.d();
        match ts_type {
            TSType::Keyword(kw) => d.text(kw.kind.as_str()),
            TSType::Literal(lit) => self.build_literal_type_doc(lit),
            TSType::Array(arr) => self.build_array_type_doc(arr),
            TSType::Union(u) => self.build_union_type_doc(u),
            TSType::Intersection(i) => self.build_intersection_type_doc(i, true),
            TSType::TypeReference(r) => {
                let mut parts: DocBuf = smallvec![self.build_entity_name_doc(&r.type_name)];
                if let Some(type_args) = &r.type_arguments {
                    // Preserve comments before type args: `Map/* c */ <string, number>`
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        r.type_name.span().end,
                        type_args.span.start,
                        CommentSpacing::Trailing,
                    ) {
                        parts.push(doc);
                    }
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                d.concat(&parts)
            }
            TSType::TypeLiteral(t) => self.build_type_literal_doc(t),
            TSType::Function(f) => self.build_function_type_doc(f),
            TSType::Constructor(c) => self.build_constructor_type_doc(c),
            TSType::Tuple(t) => self.build_tuple_type_doc(t),
            // Parenthesized types: unwrap, preserving any comments inside the parens.
            // Parent contexts (IndexedAccess, Array, TypeOperator) add parens when
            // needed based on the inner type.
            TSType::Parenthesized(p) => self.build_parenthesized_type_unwrap_doc(p),
            TSType::TypePredicate(p) => {
                let mut parts = smallvec![];
                if p.asserts {
                    // Comments between `asserts` and parameter name
                    let asserts_end = p.span.start + "asserts".len() as u32;
                    let param_start = p.parameter_name.span.start;
                    parts.push(d.text("asserts "));
                    parts.push(self.build_comments_between(
                        asserts_end,
                        param_start,
                        CommentSpacing::Trailing,
                    ));
                }
                parts.push(self.identifier_name_doc(&p.parameter_name));
                if let Some(type_ann) = &p.type_annotation {
                    // Comments between `is` keyword and the type
                    // Find `i` of `is` skipping comments (plain find("is") could match
                    // inside a comment like `/* crisis */`)
                    let param_end = p.parameter_name.span.end;
                    let type_start = type_ann.span().start;
                    let is_end = find_char_skipping_comments(
                        self.source.as_bytes(),
                        param_end as usize,
                        type_start as usize,
                        b'i',
                    )
                    .map(|i_pos| (i_pos + "is".len()) as u32);
                    // Block comment(s) in the parameter→`is` gap (`x /* c */ is T`,
                    // also `asserts x /* c */ is T` and `this /* c */ is T`) are
                    // preserved inline before `is`, matching prettier. A line comment
                    // can't occur here — a newline before `is` is a parse error — so
                    // only the block form reaches this gap; emitting nothing (the
                    // previous behavior) was silent content loss. See
                    // predicate_param_is_block_comment.
                    if let Some(is_end) = is_end {
                        let is_start = is_end - "is".len() as u32;
                        parts.push(self.build_comments_between(
                            param_end,
                            is_start,
                            CommentSpacing::Leading,
                        ));
                    }
                    // A line comment or multiline block after `is` hangs the predicate
                    // type on the next line; a single-line block comment (own-line,
                    // trailing, or glued) collapses inline (the else branch). Prettier
                    // relocates the collapsed comment before `is`. See
                    // predicate_is_line_comment / predicate_is_own_line_block_comment.
                    if let Some(is_end) = is_end
                        && self.comments_force_own_line_between(is_end, type_start)
                    {
                        let value_doc = self.build_type_doc(type_ann);
                        parts.push(d.text(" is"));
                        self.append_keyword_value_line_comments(
                            &mut parts, is_end, type_start, value_doc,
                        );
                    } else {
                        let comments_doc = is_end.map_or_else(
                            || d.empty(),
                            |is_end| {
                                self.build_comments_between(
                                    is_end,
                                    type_start,
                                    CommentSpacing::Trailing,
                                )
                            },
                        );
                        // A long union/intersection hangs after `is` (redundant parens
                        // stripped first); everything else stays inline after `is `.
                        match self.unwrap_redundant_parens(type_ann) {
                            TSType::Union(u) => {
                                let type_doc = self.build_union_type_doc(u);
                                parts.push(d.text(" is"));
                                parts.push(hang_after_operator(
                                    d,
                                    d.concat(&[comments_doc, type_doc]),
                                ));
                            }
                            TSType::Intersection(i) => {
                                parts.push(d.text(" is "));
                                parts.push(comments_doc);
                                parts.push(self.intersection_hanging_with_indent(i));
                            }
                            _ => {
                                parts.push(d.text(" is "));
                                parts.push(comments_doc);
                                parts.push(self.build_type_doc(type_ann));
                            }
                        }
                    }
                }
                d.concat(&parts)
            }
            TSType::Conditional(c) => {
                // Conditional types use width-aware wrapping:
                // When broken, ternary arms are indented:
                //   check extends extends_type
                //     ? true_type
                //     : false_type
                //
                // The outer-most conditional is wrapped in a group. Nested conditionals
                // (in true_type or false_type) are NOT wrapped in their own group - they
                // inherit breaking from the parent. This matches prettier's behavior.
                d.group(self.build_conditional_type_doc_inner(c))
            }
            TSType::Mapped(m) => self.build_mapped_type_doc(m),
            TSType::TypeOperator(o) => {
                let needs_parens = type_needs_parens_for_prefix_operator(o.type_annotation);
                // Comments between keyword and operand type
                let keyword_end = o.span.start + o.operator.as_str().len() as u32;
                let operand_start = o.type_annotation.span().start;
                // A line comment or multiline block keeps the comment with the operator
                // and hangs the operand on the next line, indented one level (the shared
                // keyword→value layout). A single-line block comment (own-line, trailing,
                // or glued) collapses inline (`keyof /* c */ B`) — matching prettier's
                // fixed point, since the prefix operators are an in-place-collapse gap,
                // not a relocation. See type_operator_keyword_line_comment /
                // type_operator_keyword_own_line_block_comment.
                if self.comments_force_own_line_between(keyword_end, operand_start) {
                    let operand_doc = self.build_type_doc(o.type_annotation);
                    let value_doc = if needs_parens {
                        d.parens(operand_doc)
                    } else {
                        operand_doc
                    };
                    let mut parts = smallvec![d.text(o.operator.as_str())];
                    self.append_keyword_value_line_comments(
                        &mut parts,
                        keyword_end,
                        operand_start,
                        value_doc,
                    );
                    return d.concat(&parts);
                }
                // `None` on the comment-free `keyof T` / `readonly T[]` — no empty child.
                let comments_doc = self.build_inline_comments_between_doc_trailing_space_opt(
                    keyword_end,
                    operand_start,
                );
                let mut parts: DocBuf = smallvec![d.text(o.operator.as_str()), d.text(" ")];
                if let Some(comments) = comments_doc {
                    parts.push(comments);
                }
                // A comment-free parenthesized union operand EXPANDS its (required) parens
                // when it breaks — `keyof (⏎\t'a' | 'b'⏎)` — instead of gluing the leading
                // `|` to the `(`, like the array-element / indexed-access-object arms. A
                // union under a prefix operator always needs parens, so the helper's parens
                // are exactly the required ones.
                if let Some(union_doc) =
                    self.build_expanded_parenthesized_union_opt(o.type_annotation)
                {
                    parts.push(union_doc);
                } else {
                    let operand_doc = self.build_type_doc(o.type_annotation);
                    if needs_parens {
                        parts.push(d.text("("));
                        parts.push(operand_doc);
                        parts.push(d.text(")"));
                    } else {
                        parts.push(operand_doc);
                    }
                }
                d.concat(&parts)
            }
            TSType::Import(i) => self.build_import_type_doc(i),
            TSType::TypeQuery(q) => {
                // Comments between `typeof` and the expression
                let typeof_end = q.span.start + "typeof".len() as u32;
                let expr_start = q.expr_name.span().start;
                // A line comment or multiline block keeps the comment with `typeof` and
                // hangs the expression on the next line (the shared keyword→value
                // layout). A single-line block comment (own-line, trailing, or glued)
                // collapses inline (`typeof /* c */ x`) like the other prefix operators
                // (in-place-collapse, not relocation).
                if self.comments_force_own_line_between(typeof_end, expr_start) {
                    let mut value_parts: DocBuf =
                        smallvec![self.build_type_query_expr_name_doc(&q.expr_name)];
                    if let Some(type_args) = &q.type_arguments {
                        let gap_start = q.expr_name.span().end;
                        if let Some(doc) = self.build_name_to_type_params_comments_opt(
                            gap_start,
                            type_args.span.start,
                            CommentSpacing::Trailing,
                        ) {
                            value_parts.push(doc);
                        }
                        value_parts.push(self.build_type_arguments_doc(type_args));
                    }
                    let value_doc = d.concat(&value_parts);
                    let mut parts = smallvec![d.text("typeof")];
                    self.append_keyword_value_line_comments(
                        &mut parts, typeof_end, expr_start, value_doc,
                    );
                    return d.concat(&parts);
                }
                let mut parts: DocBuf = smallvec![d.text("typeof ")];
                if let Some(comments) = self
                    .build_inline_comments_between_doc_trailing_space_opt(typeof_end, expr_start)
                {
                    parts.push(comments);
                }
                parts.push(self.build_type_query_expr_name_doc(&q.expr_name));
                if let Some(type_args) = &q.type_arguments {
                    // Preserve comments: `typeof fn/* c */ <string>`
                    let gap_start = q.expr_name.span().end;
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        gap_start,
                        type_args.span.start,
                        CommentSpacing::Trailing,
                    ) {
                        parts.push(doc);
                    }
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                d.concat(&parts)
            }
            TSType::IndexedAccess(i) => {
                let index_type_start = i.index_type.span().start;
                let bracket_area_start = i.object_type.span().end;
                // The access `[`, located outside comments so a `[` glyph inside a
                // comment before it (`A /* [ */[K]`) isn't mistaken for the bracket.
                let bracket_open =
                    self.find_char_outside_comments(bracket_area_start, index_type_start, b'[');
                // A comment-free parenthesized union OBJECT expands its parens when it
                // breaks (`(⏎\t| A⏎\t| B⏎)[K]`); any other object keeps the existing
                // layout. See the shared `build_expanded_parenthesized_union_opt`.
                let object_doc = self
                    .build_expanded_parenthesized_union_opt(i.object_type)
                    .unwrap_or_else(|| {
                        let needs_parens =
                            type_needs_parens_for_indexed_access_object(i.object_type);
                        let object_doc = self.build_type_doc(i.object_type);
                        if needs_parens {
                            d.concat(&[d.text("("), object_doc, d.text(")")])
                        } else {
                            object_doc
                        }
                    });
                // Comments in the object→`[` gap (`A /* c */[K]`) trail the object
                // in place; comments in the `[`→index gap (`A[/* c */ K]`) lead the
                // index — both preserved where the user placed them.
                // Both gaps break a line comment onto its own line so it can't
                // swallow the following `[`/index (the comment-aware delimiter scan
                // keeps a `[`/`]` glyph inside a comment from being read as the bracket).
                let object_comments = bracket_open
                    .map(|bp| self.build_leading_comments_break_for_line(bracket_area_start, bp));
                // A line comment (or multiline block) in the `[`→index gap breaks the
                // index onto its own line so a `//` can't swallow it
                // (indexed_access_line_comment). A single-line block comment (own-line,
                // trailing, or glued) collapses the index inline (`A[/* c */ K]`);
                // prettier relocates the comment out before `[` (`A /* c */[K]`) — see
                // indexed_access_own_line_block_comment.
                let index_comments = bracket_open.map(|bp| {
                    if self.comments_force_own_line_between(bp + 1, index_type_start) {
                        self.build_trailing_comments_hang_next(bp + 1, index_type_start)
                    } else {
                        self.build_comments_between(
                            bp + 1,
                            index_type_start,
                            CommentSpacing::Trailing,
                        )
                    }
                });
                // A comment-free union INDEX expands the bracket when it breaks:
                // `Foo[⏎\t| A⏎\t| B]` — the `]` hugs the last member (prettier's
                // `printUnionType` indent branch, `group(indent([softline, printed]))`,
                // with no trailing softline). The brackets are the delimiter, so a
                // parenthesized index union is unwrapped first — its (redundant) parens
                // strip and the bare union expands, matching prettier (the object arm
                // unwraps the same way). A comment anywhere in the `[`…`]` region keeps
                // the existing hang layout so comment placement is untouched. See
                // `type_param_fits_rhs_long`.
                let index_inner = unwrap_parenthesized(i.index_type);
                let index_expands = bracket_open.is_some_and(|bp| {
                    matches!(index_inner, TSType::Union(u) if !self.union_prints_hugged(u))
                        && !self.has_comments_to_emit_between(bp + 1, i.span.end)
                });
                let index_doc = if index_expands {
                    d.group(d.indent(d.concat(&[d.softline(), self.build_type_doc(index_inner)])))
                } else {
                    self.build_type_doc(i.index_type)
                };
                let mut parts: DocBuf = smallvec![object_doc];
                if let Some(c) = object_comments {
                    parts.push(c);
                }
                parts.push(d.text("["));
                if let Some(c) = index_comments {
                    parts.push(c);
                }
                parts.extend([index_doc, d.text("]")]);
                d.concat(&parts)
            }
            TSType::Rest(r) => {
                // Comments between `...` and the type
                let dots_end = r.span.start + "...".len() as u32;
                let type_start = r.type_annotation.span().start;
                // Break a line comment so it can't swallow the rest-element type.
                let comments_doc = self.build_trailing_comments_hang_next(dots_end, type_start);
                d.concat(&[
                    d.text("..."),
                    comments_doc,
                    self.build_type_doc(r.type_annotation),
                ])
            }
            TSType::Optional(o) => {
                let inner = self.build_type_doc_maybe_parens(
                    o.type_annotation,
                    type_needs_parens_for_optional_element,
                );
                d.concat(&[inner, d.text("?")])
            }
            TSType::NamedTupleMember(n) => {
                let mut parts = smallvec![self.identifier_name_doc(&n.label)];
                let label_end = n.label.span.end;
                let type_start = n.element_type.span().start;
                // Comments between label and `?` (e.g., `[a /* c */?: T]`)
                let after_modifier = if n.optional {
                    self.push_modifier_marker_doc(&mut parts, label_end, b'?')
                } else {
                    label_end
                };
                // Find `:` between label/`?` and type, skipping comments
                let after_colon = find_char_skipping_comments(
                    self.source.as_bytes(),
                    after_modifier as usize,
                    type_start as usize,
                    b':',
                )
                .map(|p| (p + 1) as u32); // +1 for after `:`
                // Comments between label/`?` and `:` (e.g., `[b /* c */: T]`); a line
                // comment breaks so it can't swallow the `:`.
                if let Some(after_colon) = after_colon
                    && self.has_comments_to_emit_between(after_modifier, after_colon - 1)
                {
                    parts.push(
                        self.build_leading_comments_break_for_line(after_modifier, after_colon - 1),
                    );
                }
                // Comments between `:` and the element type; a line comment breaks so it
                // can't swallow the type.
                let comments_doc = after_colon.map_or_else(
                    || d.empty(),
                    |after_colon| self.build_trailing_comments_hang_next(after_colon, type_start),
                );
                // A long union/intersection element hangs after `:` (redundant parens
                // stripped first); everything else stays inline after `: `.
                match self.unwrap_redundant_parens(n.element_type) {
                    TSType::Union(u) => {
                        let type_doc = self.build_union_type_doc(u);
                        parts.push(d.text(":"));
                        parts.push(hang_after_operator(d, d.concat(&[comments_doc, type_doc])));
                    }
                    TSType::Intersection(i) => {
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(self.intersection_hanging_with_indent(i));
                    }
                    _ => {
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(self.build_type_doc(n.element_type));
                    }
                }
                d.concat(&parts)
            }
            TSType::Infer(i) => {
                // Comments between `infer` and the type parameter name
                let infer_end = i.span.start + "infer".len() as u32;
                let name_start = i.type_parameter.name.span.start;
                // Delegate the name + optional `extends C` constraint to the shared
                // type-parameter doc builder — prettier's `printInferType` is
                // `["infer ", print("typeParameter")]`, so an infer constraint lays
                // out identically to a `<T extends C>` declaration constraint.
                let type_param_doc = self.build_type_parameter_doc(&i.type_parameter);
                // A line comment or multiline block keeps the comment with `infer` and
                // hangs the name on the next line, indented one level (the shared
                // keyword→value layout). A single-line block comment (own-line, trailing,
                // or glued) collapses inline (`infer /* c */ R`) — matching prettier's
                // fixed point, an in-place-collapse gap. See infer/keyword_line_comment /
                // infer/keyword_own_line_block_comment.
                if self.comments_force_own_line_between(infer_end, name_start) {
                    let mut parts: DocBuf = smallvec![d.text("infer")];
                    self.append_keyword_value_line_comments(
                        &mut parts,
                        infer_end,
                        name_start,
                        type_param_doc,
                    );
                    return d.concat(&parts);
                }
                // A block comment glued to the name stays inline (`infer /* c */ R`).
                let comments_doc = self.build_trailing_comments_hang_next(infer_end, name_start);
                d.concat(&[d.text("infer "), comments_doc, type_param_doc])
            }
            TSType::ThisType(_) => d.text("this"),
        }
    }

    /// Returns true if there's a line comment between `(` and the inner type
    /// of a parenthesized type (e.g., `(// leading\n T)`).
    ///
    /// ⚠️ **Shallow — checks only THIS paren's own one-level gap.** Correct only
    /// when the caller retains this exact paren (the `TSType::Union(_)`-guarded
    /// paren-union member callers). For a paren the caller will STRIP — where a
    /// double-nested `((// c\n T))` hides the comment one layer deeper, between the
    /// two `(`s this window never reaches — use the deep
    /// [`Self::stripped_paren_has_leading_line_comment`] instead.
    pub(in crate::printer) fn paren_has_leading_line_comment(
        &self,
        p: &TSParenthesizedType<'_>,
    ) -> bool {
        self.has_line_comments_between(p.span.start + 1, p.type_annotation.span().start)
    }

    // TODO: an adjacent, deeper bug class remains unfixed — the keyword→value hang
    // positions (`as`/`satisfies` cast, `: T` return / annotation, mapped-type value,
    // type-parameter `=` default, predicate `is`) gate on `comments_force_own_line_between`
    // over the OUTER paren, not this deep window, so `((// c\n X))` — and even the single
    // `(// c\n X)` — places non-idempotently there. Those sites build the value via
    // `build_type_doc(paren)` (no paren-strip), so the fix is a separate, larger change,
    // not just routing through the deep helper below.

    /// Deep analog of [`Self::paren_has_leading_line_comment`]: does a possibly
    /// multiply-nested redundant paren shell (`((// c\n X))`) hold a **relocatable**
    /// leading line-comment run — one it is safe to hoist while stripping the shell?
    /// True exactly when [`Self::stripped_paren_leading_line_comments`] returns a run.
    ///
    /// This is the predicate every caller that will **strip** the paren layers wants:
    /// the comment can't stay "inside" parens that don't survive, so it must relocate
    /// with the strip. Using the shallow window here was the bug — a double-nested
    /// paren's comment fell between the two `(`s and the caller relocated nothing,
    /// placing it non-idempotently. Mirrors `build_union_type_doc`'s
    /// `has_paren_inner_leading_line_comments` router probe.
    pub(in crate::printer) fn stripped_paren_has_leading_line_comment(
        &self,
        ty: &TSType<'_>,
    ) -> bool {
        // Fail-fast on the cheap gates (this runs unconditionally, 3× per conditional
        // type): a non-paren, or a paren with no leading line comment, never allocates
        // the collector's `CommentVec`. Both gates are implied by a non-empty run, so
        // adding them can't change the result — only skip the collect + trailing scan.
        matches!(ty, TSType::Parenthesized(_))
            && self.has_line_comments_between(
                ty.span().start + 1,
                unwrap_parenthesized(ty).span().start,
            )
            && !self.stripped_paren_leading_line_comments(ty).is_empty()
    }

    /// Collect the leading line-comment run in a stripped paren shell — the deep-window
    /// collector paired with [`Self::stripped_paren_has_leading_line_comment`]. Scans
    /// the whole discarded shell, from the OUTERMOST `(` to the fully-unwrapped inner
    /// type's start (`unwrap_parenthesized`), where the shallow predicate sees only one
    /// paren's own gap.
    ///
    /// Returns the run ONLY when stripping the shell would relocate it losslessly: the
    /// leading gap holds ≥1 comment, ALL line comments, AND there is no comment in the
    /// trailing gap between the inner type and the outermost `)`. A block comment in
    /// the leading gap, or any trailing comment, would be silently DROPPED by the
    /// stripped-inner render the caller uses — so the run is declined (empty), and the
    /// caller builds the parenthesized type normally, preserving every comment in
    /// place. Mirrors [`Self::stripped_redundant_paren_leading_line_comments`] (its
    /// union analog), minus the union-specific redundancy check the caller's context
    /// already implies. Empty when `ty` is not a parenthesized type.
    pub(in crate::printer) fn stripped_paren_leading_line_comments(
        &self,
        ty: &TSType<'_>,
    ) -> CommentVec<'_> {
        if !matches!(ty, TSType::Parenthesized(_)) {
            return smallvec![];
        }
        let inner = unwrap_parenthesized(ty);
        let lead: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, ty.span().start + 1, inner.span().start)
                .collect();
        // Non-empty + all line comments ⇒ ≥1 leading line comment and no block comment
        // in the leading gap; the trailing check rules out a comment between the inner
        // and the outermost `)`.
        if !lead.is_empty()
            && lead.iter().all(|c| !c.is_block)
            && !self.has_comments_to_emit_between(inner.span().end, ty.span().end - 1)
        {
            return lead;
        }
        smallvec![]
    }

    /// Build a complete import type: the `import(<specifier>)` call plus its
    /// optional `.qualifier` and `<type args>`, preserving comments at each
    /// boundary. Shared by `TSType::Import` and the `typeof import(...)` form
    /// (`TSTypeQueryExprName::Import`), which must format identically.
    pub(in crate::printer) fn build_import_type_doc(&self, i: &TSImportType<'_>) -> DocId {
        let d = self.d();
        // Closing `)` of the `import(...)` call, skipping any inside comments.
        let after_args = i
            .options
            .as_ref()
            .map_or(i.argument.span.end, |o| o.span().end);
        let paren_close = self
            .find_char_outside_comments(after_args, i.span.end, b')')
            .unwrap_or(after_args);

        let mut parts: DocBuf = smallvec![self.build_import_type_call_doc(i, paren_close)];
        if let Some(qualifier) = &i.qualifier {
            // Comments between `)` and qualifier (e.g. `import('a') /* c */ .Foo`); a
            // line comment breaks so it can't swallow the qualifier.
            let dot_area_start = paren_close + 1;
            let qualifier_start = qualifier.span().start;
            parts.push(d.text("."));
            parts.push(self.build_trailing_comments_hang_next(dot_area_start, qualifier_start));
            parts.push(self.build_entity_name_doc(qualifier));
        }
        if let Some(type_args) = &i.type_arguments {
            // Preserve comments before type args: `import("a").Foo/* c */ <string>`
            let gap_start = i
                .qualifier
                .as_ref()
                .map_or(paren_close + 1, |q| q.span().end);
            if let Some(doc) = self.build_name_to_type_params_comments_opt(
                gap_start,
                type_args.span.start,
                CommentSpacing::Trailing,
            ) {
                parts.push(doc);
            }
            parts.push(self.build_type_arguments_doc(type_args));
        }
        d.concat(&parts)
    }

    /// Build the `import(<specifier>)` call portion of an import type, preserving
    /// comments between `import(` and the specifier (leading) and between the
    /// specifier and `)` (trailing). Leading comments go through the shared
    /// `build_paren_leading_value_doc` (also used by the dynamic-import expression in
    /// `calls/import_expr.rs`). Qualifier / type arguments are appended by the caller.
    ///
    /// - leading line / own-line block comment → break the parens multiline
    /// - inline block comment → stay inline (`import(/* c */ 'a')`)
    /// - trailing line comment → break multiline; trailing block → inline
    fn build_import_type_call_doc(&self, i: &TSImportType<'_>, paren_close: u32) -> DocId {
        let d = self.d();
        let open_paren_end = i.span.start + "import(".len() as u32;
        let arg_start = i.argument.span.start;
        let arg_end = i.argument.span.end;
        let literal_doc = self.build_literal_doc(&i.argument);

        // Options present: keep the inline `import('a', {...})` layout, preserving
        // any leading comments before the specifier.
        if let Some(options) = &i.options {
            let arg_doc = match self.build_rhs_comments_opt(open_paren_end, arg_start) {
                Some(lead) => d.concat(&[lead, literal_doc]),
                None => literal_doc,
            };
            return d.concat(&[
                d.text("import("),
                arg_doc,
                d.text(", "),
                self.build_expression_doc(options),
                d.text(")"),
            ]);
        }

        // Leading comments between `import(` and the specifier.
        let (arg_doc, leading_forces_break) =
            self.build_paren_leading_value_doc(open_paren_end, arg_start, literal_doc);

        // Trailing comments between the specifier and `)`.
        let has_trailing = self.has_comments_to_emit_between(arg_end, paren_close);
        let has_trailing_line = self.has_line_comments_between(arg_end, paren_close);

        let mut inner = smallvec![arg_doc];
        if has_trailing {
            let pc = PartitionedComments::new(
                self.comments,
                self.comment_line_breaks,
                arg_end,
                paren_close,
            );
            pc.emit_trailing_comments(&mut inner, self);
        }
        let inner = d.concat(&inner);

        if leading_forces_break || has_trailing_line {
            // Line / own-line comments force the parens to break across lines.
            d.concat(&[
                d.text("import("),
                d.indent(d.concat(&[d.hardline(), inner])),
                d.hardline(),
                d.text(")"),
            ])
        } else {
            // Block comments only (or none) — stay inline.
            d.concat(&[d.text("import("), inner, d.text(")")])
        }
    }

    /// Whether a `TSParenthesizedType` carries comments inside its parens, as
    /// `(has_leading, has_trailing)` flags — leading = between `(` and the inner
    /// type, trailing = between the inner type and `)`. Used both to decide
    /// whether redundant parens can be stripped and to emit the comments in place
    /// when they can't.
    pub(in crate::printer) fn paren_inner_comment_flags(
        &self,
        p: &TSParenthesizedType<'_>,
    ) -> (bool, bool) {
        let inner = p.type_annotation.span();
        (
            self.has_comments_to_emit_between(p.span.start, inner.start),
            self.has_comments_to_emit_between(inner.end, p.span.end),
        )
    }

    /// Unwrap redundant, comment-free `TSParenthesizedType` layers to find the
    /// effective inner type for a layout decision. Parens around a union /
    /// intersection in type-alias-RHS, cast (`as` / `satisfies`), return-type,
    /// and type-member positions are redundant — prettier strips them — so a
    /// `(union)` / `(intersection)` should get the same break layout as the bare
    /// form (leading `| ` for unions, hanging indent for intersections) rather
    /// than hanging inline. Stops at a paren that carries comments — those are
    /// preserved in place by `build_parenthesized_type_unwrap_doc`.
    pub(in crate::printer) fn unwrap_redundant_parens<'t>(
        &self,
        ty: &'t TSType<'t>,
    ) -> &'t TSType<'t> {
        match ty {
            TSType::Parenthesized(p) if self.paren_inner_comment_flags(p) == (false, false) => {
                self.unwrap_redundant_parens(p.type_annotation)
            }
            other => other,
        }
    }

    /// Unwrap a parenthesized type, preserving any comments inside the parens.
    ///
    /// Block comments are emitted inline: `(/* c */ a)` → `/* c */ a`
    /// Line comments use `line_suffix` to defer to end of the rendered line,
    /// plus `break_parent` to force the enclosing union/intersection group to break:
    /// `(a // comment\n) | b` → `| a // comment\n| b`
    /// `(a // comment\n) & b` → `a & // comment\nb`
    fn build_parenthesized_type_unwrap_doc(&self, p: &TSParenthesizedType<'_>) -> DocId {
        let d = self.d();
        let paren_open = p.span.start;
        let inner_start = p.type_annotation.span().start;
        let inner_end = p.type_annotation.span().end;
        let paren_close = p.span.end;
        let (has_leading, has_trailing) = self.paren_inner_comment_flags(p);
        if !has_leading && !has_trailing {
            return self.build_type_doc(p.type_annotation);
        }

        let mut parts: DocBuf = DocBuf::new();
        let mut needs_break = false;

        // Leading comments: between `(` and inner type
        if has_leading {
            for comment in comments_to_emit_in_range(self.comments, paren_open, inner_start) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                } else {
                    // Line comment before inner type: emit inline + hardline.
                    // A line comment must terminate at end-of-line; using line_suffix
                    // here would defer it past the end of the enclosing construct
                    // and can produce invalid output (e.g., `[// leading a, b]`).
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.hardline());
                    needs_break = true;
                }
            }
        }

        parts.push(self.build_type_doc(p.type_annotation));

        // Trailing comments: between inner type and `)`
        if has_trailing {
            for comment in comments_to_emit_in_range(self.comments, inner_end, paren_close) {
                if comment.is_block {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                } else {
                    // Line comment after inner type: defer to end of line, force break
                    let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                    parts.push(d.line_suffix(suffix));
                    needs_break = true;
                }
            }
        }

        if needs_break {
            parts.push(d.break_parent());
        }
        d.concat(&parts)
    }
}
