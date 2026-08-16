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

// The array-suffix layout verdict, shared with the type-alias `=` gate
// (`statements/type_declarations.rs`) so the emitter and the gate cannot disagree.
pub(in crate::printer) use composite::ArraySuffixLayout;

// Re-export for submodules to use `super::X` instead of `super::super::X`
pub(super) use super::StandaloneGlue;
pub(super) use super::comments::BlankRule;
pub(super) use super::{CommentFilter, CommentSpacing, Printer};

use crate::ast::internal::{TSImportType, TSParenthesizedType, TSType};
use crate::printer::CommentVec;
use crate::printer::calls::{ImportOptionsArg, build_import_args_comment_layout};
use crate::printer::layout::hang_after_operator;
use helpers::type_needs_parens_for_indexed_access_object;
use helpers::type_needs_parens_for_optional_element;
use helpers::type_needs_parens_for_prefix_operator;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{find_char_skipping_comments, skip_comment};

/// How [`Printer::with_stripped_paren_trailing`] emits a trailing **block**
/// comment lifted from a stripped shell's gap (a trailing **line** comment
/// always defers — a `//` must end its line).
#[derive(Debug, Clone, Copy)]
pub(in crate::printer) enum TrailingBlock {
    /// Trail inline (`X /* c */`) — a **type** position, where the enclosing
    /// construct keeps a value-trailing block before its terminator, so inline
    /// is that position's fixed point.
    Inline,
    /// Defer via `line_suffix` past the statement terminator — a **value**
    /// position (an `as`/`satisfies` cast), matching the declarator's own
    /// value→`;` trailing-comment handling.
    Deferred,
}

/// A resolved keyword→value head — see [`Printer::keyword_value_head`].
pub(in crate::printer) struct KeywordValueHead<'t> {
    /// The head gap's start — the keyword's end. `None` only at a site that PROVED its
    /// whole construct comment-free and therefore never located the keyword (the
    /// type-parameter constraint / default, whose `extends` / `=` byte scan is skipped
    /// on that path — see [`KeywordValueHead::without_gap`]): with no gap there is no
    /// comment to emit and no directive to honor.
    pub(in crate::printer) gap_start: Option<u32>,
    /// Whether an alone-on-line directive in the head gap freezes the child verbatim.
    pub(in crate::printer) frozen: bool,
    /// The head gap's end — the window the caller's gate measures AND its leading-run
    /// emitter claims. The child's own start under a freeze, else the paren-strip hang
    /// seam's (possibly widened) start.
    pub(in crate::printer) value_start: u32,
    /// The head's own child, pre-strip — the freeze slice and the trailing-lift anchor.
    child: &'t TSType<'t>,
    /// The type to build: the child itself under a freeze, else the hang seam's
    /// (possibly paren-stripped) inner.
    pub(in crate::printer) value_type: &'t TSType<'t>,
}

impl<'t> KeywordValueHead<'t> {
    /// The head for a site that proved its construct comment-free and so never located
    /// its keyword: no gap, hence no freeze (a directive is a comment) and no
    /// paren-strip probe (the seam only fires on a leading LINE comment) — the window
    /// collapses to the child's own start and the value is the child itself. Scan-free
    /// by construction, which is the point: it is the head shape of the path that
    /// deliberately pays nothing.
    pub(in crate::printer) fn without_gap(child: &'t TSType<'t>) -> Self {
        Self {
            gap_start: None,
            frozen: false,
            value_start: child.span().start,
            child,
            value_type: child,
        }
    }
}

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
            // Default trailing-prefix context; own-line element callers (tuple) invoke
            // `build_intersection_type_doc` directly with `own_line = true`.
            TSType::Intersection(i) => self.build_intersection_type_doc(i, true, false),
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
                    // An alone-on-line format-ignore directive in the `is`→type gap
                    // freezes a non-composite predicate type verbatim
                    // (`single_child_frozen`; a union/intersection type declines and
                    // freezes via its own leading-run walk). The frozen path keeps the
                    // UNWIDENED window — an in-shell directive stays on the ordinary
                    // paths — and the directive keeps its own line (an `is`-trailing
                    // placement is inert, so the relocated form would lose the freeze
                    // on the second pass). `head.frozen` joins the routing below so a
                    // block-spelling alone-on-line directive takes the own-line branch.
                    // A redundant paren shell with a leading line-comment run
                    // (`x is (// c\n T)`, and the double-nested form) strips to the same
                    // hang as bare `x is // c\n T`; the shared keyword→value seam routes
                    // it so the paren form is idempotent (the outer paren would otherwise
                    // hide the comment from the gate below). A mixed / trailing shell
                    // hoists losslessly too — the trailing comment via
                    // `build_hang_value_doc`.
                    let head = is_end.map(|is_end| self.keyword_value_head(is_end, type_ann));
                    // A line comment or multiline block after `is` hangs the predicate
                    // type on the next line; a single-line block comment (own-line,
                    // trailing, or glued) collapses inline (the else branch). Prettier
                    // relocates the collapsed comment before `is`. See
                    // predicate_is_line_comment / predicate_is_own_line_block_comment.
                    if let Some((is_end, head)) = is_end.zip(head.as_ref())
                        && (head.frozen
                            || self.comments_force_own_line_between(is_end, head.value_start))
                    {
                        // Type position: a trailing block lifted from the shell trails
                        // the type inline before the body `{`.
                        let value_doc = self.build_keyword_value_doc(head, TrailingBlock::Inline);
                        parts.push(d.text(" is"));
                        self.append_keyword_value_line_comments(
                            &mut parts,
                            is_end,
                            head.value_start,
                            value_doc,
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
                // A redundant paren shell with a leading line-comment run
                // (`keyof (// c\n T)`) strips to the same hang as bare `keyof // c\n T`;
                // route the operand through the shared keyword→value seam so the paren form
                // is idempotent (the outer paren would otherwise hide the comment from the
                // gate). `needs_parens` is recomputed on the *unwrapped* operand: a
                // semantically-required paren (a union under `keyof`) is re-added rather
                // than dropped, a redundant one is shed. A mixed / trailing shell hoists
                // losslessly too — the trailing comment via `with_stripped_paren_trailing`,
                // applied after the operand's own parens are (re-)added.
                //
                // ⚠️ Except a shell the trailing-run rule RETAINS. The prefix operator is
                // the one REQUIRED-pair position that also has a keyword→value hang seam,
                // and taking the hang there stripped a shell whose pair the operand needs
                // anyway, re-added it bare, and lifted the `//` out past the `;` —
                // `keyof (// c1⏎ B extends C ? D : E // c2)` printing `keyof // c1⏎(B
                // extends C ? D : E); // c2`, where a second trailing `//` then welds onto
                // the first. Its four sibling required-pair positions (optional element,
                // conditional check / `extends`, array element, indexed-access object)
                // have no hang seam and so already retain. Declining here lands this one
                // on the same emitter they use: the gap left to measure is `keyof`→`(`,
                // which holds nothing, so the inline path below runs and
                // `build_required_paren_operand_doc` hands the shell to its own builder.
                let (operand_hang_start, operand_hang_type) =
                    if self.paren_retains_for_trailing_run(o.type_annotation) {
                        (o.type_annotation.span().start, o.type_annotation)
                    } else {
                        self.keyword_value_stripped_paren_hang(o.type_annotation)
                    };
                if self.comments_force_own_line_between(keyword_end, operand_hang_start) {
                    let operand_doc = self.build_type_doc(operand_hang_type);
                    let value_doc = if type_needs_parens_for_prefix_operator(operand_hang_type) {
                        d.parens(operand_doc)
                    } else {
                        operand_doc
                    };
                    // Type position: a trailing block lifted from the shell trails the
                    // operand inline.
                    let value_doc = self.with_stripped_paren_trailing(
                        value_doc,
                        o.type_annotation,
                        operand_hang_type,
                        TrailingBlock::Inline,
                    );
                    let mut parts = smallvec![d.text(o.operator.as_str())];
                    self.append_keyword_value_line_comments(
                        &mut parts,
                        keyword_end,
                        operand_hang_start,
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
                    parts.push(if needs_parens {
                        self.build_required_paren_operand_doc(o.type_annotation, |operand| {
                            d.concat(&[d.text("("), operand, d.text(")")])
                        })
                    } else {
                        self.build_type_doc(o.type_annotation)
                    });
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
                        if type_needs_parens_for_indexed_access_object(i.object_type) {
                            self.build_required_paren_operand_doc(i.object_type, |object| {
                                d.concat(&[d.text("("), object, d.text(")")])
                            })
                        } else {
                            self.build_type_doc(i.object_type)
                        }
                    });
                // Comments in the object→`[` gap (`A /* c */[K]`) trail the object
                // in place; comments in the `[`→index gap (`A[/* c */ K]`) lead the
                // index — both preserved where the user placed them.
                //
                // The object→`[` gap can hold only a single-line block: a type's index
                // suffix may not follow a line break, so a `//` (or a multiline block)
                // there means the source never parsed as an indexed access at all
                // (`type X = A // c⏎[K];` is `type X = A;` plus an `ArrayExpression`
                // statement). Hence the plain inline run — no break to keep a comment
                // from swallowing the `[`. The `[`→index gap below, which *can* hold a
                // line comment, takes the hanging emitter instead.
                let object_comments = bracket_open.and_then(|bp| {
                    debug_assert!(
                        !self.has_line_comments_between(bracket_area_start, bp),
                        "a line comment before `[` means this never parsed as an indexed access"
                    );
                    self.build_inline_comments_between_doc_opt(bracket_area_start, bp)
                });
                // An alone-on-line format-ignore directive in the `[`→index gap stays
                // OWN-LINE inside the brackets — the trailing-hang emitter below would
                // glue it to the `[` (`[// prettier-ignore`), an inert placement that
                // loses the freeze on the second pass — and freezes a non-composite
                // index verbatim (`single_child_frozen`; a composite index declines and
                // freezes via its own leading-run walk). The brackets expand around the
                // run: `A[⏎⇥// prettier-ignore⏎⇥K⏎]`.
                if let Some(bp) = bracket_open
                    && self.member_gap_frozen(bp + 1, index_type_start)
                {
                    let index_doc = self.build_routed_child_doc(i.index_type);
                    let mut parts: DocBuf = smallvec![object_doc];
                    if let Some(c) = object_comments {
                        parts.push(c);
                    }
                    parts.push(d.text("["));
                    self.append_keyword_value_line_comments(
                        &mut parts,
                        bp + 1,
                        index_type_start,
                        index_doc,
                    );
                    // Comments in the index→`]` gap trail the index line (a line
                    // comment rides `line_suffix`, flushing before the `]`'s
                    // hardline) — this route claims the whole bracket interior, so
                    // nothing here may go unemitted.
                    //
                    // `indent`, because this route already expanded the brackets and put
                    // the index one level in: a second comment in the run takes its own
                    // line and belongs in that same interior column, not out at the `[`'s.
                    // Inert for the run's first comment, which trails the index's line.
                    let mut gap: DocBuf = DocBuf::new();
                    self.push_trailing_comments_in_range(
                        &mut gap,
                        i.index_type.span().end,
                        i.span.end,
                    );
                    if !gap.is_empty() {
                        parts.push(d.indent(d.concat(&gap)));
                    }
                    parts.push(d.hardline());
                    parts.push(d.text("]"));
                    return d.concat(&parts);
                }
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
                // Comments in the index→`]` gap trail the index and STAY INSIDE the
                // brackets — the treatment every other bracketed type region already
                // gives its own trailing gap (a type literal's `}`, a type-argument
                // list's `>`, a tuple's `]`, a function type's `)`, and the retained
                // paren shell), so the construct answers the question one way. A
                // **line** comment there runs to end of line, so the `]` cannot follow
                // it: the brackets open, the index sits one level in, and the run
                // takes that same interior column. Letting the comment ride out to
                // end-of-line instead re-bound it from the index to the whole
                // statement and landed it on a line that may already hold one, where
                // the two weld irreversibly — see
                // [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
                // §Comment relocation. The expanding-union index layout is
                // comment-gated, so it never carries one of these.
                let gap_start = i.index_type.span().end;
                if self.has_line_comments_between(gap_start, i.span.end) {
                    let mut inner: DocBuf = DocBuf::new();
                    if let Some(c) = index_comments {
                        inner.push(c);
                    }
                    inner.push(index_doc);
                    self.push_trailing_comments_in_range(&mut inner, gap_start, i.span.end);
                    parts.push(d.indent_hardline(d.concat(&inner)));
                    parts.push(d.hardline());
                } else {
                    if let Some(c) = index_comments {
                        parts.push(c);
                    }
                    parts.push(index_doc);
                    self.push_trailing_comments_in_range(&mut parts, gap_start, i.span.end);
                }
                parts.push(d.text("]"));
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
                // The optional element is the SOLE emitter of its operand shell's leading
                // line-comment run — no union-member relocation, no stripped-shell hang
                // upstream — so it takes the variant that keeps the run inside the parens
                // rather than the shared entry point, which declines it.
                let inner = self.build_optional_element_type_doc(
                    o.type_annotation,
                    type_needs_parens_for_optional_element,
                );
                let mut parts: DocBuf = smallvec![inner];
                // Comments in the element→`?` gap (`[T /* c */?]`) take the same
                // landing the `NamedTupleMember` arm below gives the label→`?` gap
                // (`[a /* c */?: T]`) — previously unclaimed by any emitter here, a
                // silent drop. The element's span end is the gap's left edge: a
                // parenthesized operand keeps its `TSType::Parenthesized` wrapper, so
                // the scan starts after the `)`, and an in-paren comment stays inside.
                // Only a *same-line block* comment can be here — the `?` is a
                // `[no LineTerminator here]` position, so the emitter's line-comment
                // branch is unreachable from this caller (`parse_tuple_element_inner`).
                self.push_modifier_marker_doc(&mut parts, o.type_annotation.span().end, b'?');
                d.concat(&parts)
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
                // The whole `: element` tail, built before the label/`?`→`:` gap is
                // emitted so a line comment there can wrap it in the continuation
                // indent below.
                //
                // A format-ignore directive in the `:`→element gap freezes a
                // non-composite element type (`single_child_frozen`; a
                // union/intersection element declines and freezes via its own walk —
                // the union hang below already keeps its comments own-line). The
                // directive — alone on its line by the placement floor — keeps its own
                // line: the default emission below trails the first comment after `:`,
                // a placement that reads as inert on the second pass.
                let frozen_tail = after_colon.and_then(|after_colon| {
                    self.single_child_frozen(after_colon, n.element_type)
                        .then(|| {
                            let frozen_doc = self.build_frozen_single_child_doc(n.element_type);
                            let mut tail: DocBuf = smallvec![d.text(":")];
                            self.append_keyword_value_line_comments(
                                &mut tail,
                                after_colon,
                                type_start,
                                frozen_doc,
                            );
                            d.concat(&tail)
                        })
                });
                let tail = frozen_tail.unwrap_or_else(|| {
                    // Comments between `:` and the element type; a line comment breaks so it
                    // can't swallow the type.
                    let comments_doc = after_colon.map_or_else(
                        || d.empty(),
                        |after_colon| {
                            self.build_trailing_comments_hang_next(after_colon, type_start)
                        },
                    );
                    // A long union/intersection element hangs after `:` (redundant parens
                    // stripped first); everything else stays inline after `: `.
                    match self.unwrap_redundant_parens(n.element_type) {
                        TSType::Union(u) => {
                            let type_doc = self.build_union_type_doc(u);
                            d.concat(&[
                                d.text(":"),
                                hang_after_operator(d, d.concat(&[comments_doc, type_doc])),
                            ])
                        }
                        TSType::Intersection(i) => d.concat(&[
                            d.text(": "),
                            comments_doc,
                            self.intersection_hanging_with_indent(i),
                        ]),
                        _ => d.concat(&[
                            d.text(": "),
                            comments_doc,
                            self.build_type_doc(n.element_type),
                        ]),
                    }
                });
                // Comments between label/`?` and `:` (e.g., `[b /* c */: T]`). A **line**
                // comment keeps the comment trailing the head and drops the whole
                // `: element` tail to a continuation line indented one level (the uniform
                // forced-continuation indent) — emitting the `:` inline after a `//` would
                // swallow it. A block comment stays in its authored gap, glued to the `:`,
                // which is prettier's form too. See conformance_prettier.md §Uniform
                // Forced-Continuation Indent.
                if let Some(colon_pos) = after_colon.map(|after_colon| after_colon - 1)
                    && self.has_comments_to_emit_between(after_modifier, colon_pos)
                {
                    if self.has_line_comments_between(after_modifier, colon_pos) {
                        parts.push(self.build_continuation_indent(after_modifier, colon_pos, tail));
                        return d.concat(&parts);
                    }
                    parts.push(self.build_comments_between(
                        after_modifier,
                        colon_pos,
                        CommentSpacing::Leading,
                    ));
                }
                parts.push(tail);
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
        // The narrow analog is exactly the hang predicate (a paren shell with a leading
        // line comment) PLUS the content-loss check that the run is safe to hoist — no
        // leading block, no trailing comment (`stripped_paren_leading_line_comments`
        // returns empty otherwise). The hang predicate's cheap `matches!` + line-comment
        // scan fail-fast before the collector's `CommentVec` allocates (this runs
        // unconditionally, 3× per conditional type), so composing stays as cheap as the
        // hand-inlined gates were.
        self.stripped_paren_hang_has_leading_line_comment(ty)
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
    /// place. Empty when `ty` is not a parenthesized type.
    ///
    /// The union counterpart is [`Self::stripped_redundant_paren_member_leading_run`],
    /// deliberately WIDER on three axes: it hoists the full block+line run (not line-only),
    /// tolerates a trailing comment (relocated separately onto the member), and adds the
    /// union-specific redundancy check this caller's context already implies. This narrow
    /// form serves the conditional-`extends` and intersection-first-member callers.
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

    /// Hang-seam analog of [`Self::stripped_paren_has_leading_line_comment`], but
    /// **wider**: true when a possibly multiply-nested redundant paren shell holds a
    /// leading **line** comment anywhere in its (deep) leading gap — the hang trigger —
    /// *regardless* of whether it also carries a leading **block** comment (a mixed
    /// shell, `(/* b */ // c\n X)`) or a **trailing** comment (`(// c\n X /* t */)`).
    ///
    /// The narrower predicate declines those two shapes to avoid dropping the extra
    /// comment when the caller renders only the stripped inner; the hang seam instead
    /// hoists the whole run losslessly — the leading block + line via the caller's own
    /// leading-comment emitter (the gap window widens to the unwrapped inner's start,
    /// which spans the stripped parens), the trailing comment via
    /// [`Self::with_stripped_paren_trailing`]. So the seam only needs to know a line
    /// comment forces the hang; block-leading and trailing no longer decline it.
    ///
    /// Kept separate from the narrow predicate so the union-member / conditional-`extends`
    /// callers of the `stripped_*_leading_line_comments` pair — which retain the paren and
    /// preserve every comment *in place* — are unaffected.
    ///
    /// Read by the hang seam as "a line comment forces the hang", and by
    /// [`Self::build_open_required_paren_doc`]'s gate as the same underlying fact — the
    /// deep leading gap holds a `//` — so the two cannot drift on where that gap ends.
    pub(in crate::printer) fn stripped_paren_hang_has_leading_line_comment(
        &self,
        ty: &TSType<'_>,
    ) -> bool {
        matches!(ty, TSType::Parenthesized(_))
            && self.has_line_comments_between(
                ty.span().start + 1,
                unwrap_parenthesized(ty).span().start,
            )
    }

    /// The shared keyword→value seam for a hang position whose caller strips redundant
    /// parens off `value` before laying it out (`as`/`satisfies` cast, `: T` annotation,
    /// mapped-type value, type-parameter `=` default / `extends` constraint, predicate
    /// `is`, prefix operators): if `value` is a redundant paren shell whose leading gap
    /// holds a line comment (mixed and trailing shells included), return the
    /// fully-unwrapped inner type and its start, so the leading run falls into the
    /// keyword→value gap and hangs idempotently (the same fixed point the bare,
    /// paren-free form already settles on); otherwise return `value` and its own start,
    /// unchanged.
    ///
    /// Losslessness: the caller's leading-comment emitter (fed the widened gap window)
    /// prints the leading block + line run, and [`Self::with_stripped_paren_trailing`]
    /// prints any comment in the shell's trailing gap — so no comment is dropped by the
    /// strip. Gated by [`Self::stripped_paren_hang_has_leading_line_comment`].
    pub(in crate::printer) fn keyword_value_stripped_paren_hang<'t>(
        &self,
        value: &'t TSType<'t>,
    ) -> (u32, &'t TSType<'t>) {
        if self.stripped_paren_hang_has_leading_line_comment(value) {
            let inner = unwrap_parenthesized(value);
            (inner.span().start, inner)
        } else {
            (value.span().start, value)
        }
    }

    /// Append the trailing comment lifted out of a stripped redundant-paren shell to an
    /// already-built hung value doc. A no-op unless `original` is a paren shell the hang
    /// seam stripped to `inner` (`original.span() != inner.span()`) that carries a
    /// comment in its trailing gap `(inner.end, original.end)` — the gap the leading-run
    /// emitters ([`Self::append_keyword_value_line_comments`] et al.) never reach.
    ///
    /// A trailing **line** comment always uses `line_suffix` (a `//` must end its line);
    /// a trailing **block** comment follows `trailing_block` — see [`TrailingBlock`] for
    /// the position rationale.
    /// Mirrors [`Self::build_parenthesized_type_unwrap_doc`]'s trailing arm.
    ///
    /// ⚠️ The run is emitted by [`Self::push_trailing_comments_in_range`], the shared
    /// trailing-gap seam, whose policy this gap's [`TrailingBlock::Inline`] spelling
    /// exactly is — never by a loop of its own. Open-coding it dropped the seam's own-line
    /// rule, and back-to-back `line_suffix`es WELD: `(// c⏎ T // t⏎ // x)` lifted
    /// `// t // x` onto one line, the second `//` becoming text of the first. The
    /// [`TrailingBlock::Deferred`] arm below is the one thing the seam cannot express — a
    /// **block** deferring past a value position's terminator — and it carries the same
    /// own-line rule for the same reason.
    pub(in crate::printer) fn with_stripped_paren_trailing(
        &self,
        value_doc: DocId,
        original: &TSType<'_>,
        inner: &TSType<'_>,
        trailing_block: TrailingBlock,
    ) -> DocId {
        // Not a stripped shell → nothing was lifted out of a trailing gap.
        if original.span() == inner.span() {
            return value_doc;
        }
        let trailing_start = inner.span().end;
        let trailing_end = original.span().end;
        if !self.has_comments_to_emit_between(trailing_start, trailing_end) {
            return value_doc;
        }
        let d = self.d();
        let mut parts: DocBuf = smallvec![value_doc];
        let needs_break = match trailing_block {
            TrailingBlock::Inline => {
                self.push_trailing_comments_in_range(&mut parts, trailing_start, trailing_end)
            }
            TrailingBlock::Deferred => {
                let mut has_line_comment = false;
                // The in-source cursor the own-line question is asked against — it
                // advances over every comment emitted here, deferred or not.
                let mut prev_end = trailing_start;
                for comment in
                    comments_to_emit_in_range(self.comments, trailing_start, trailing_end)
                {
                    // Everything defers at a value position, so the break that separates
                    // two of them must ride INSIDE the `line_suffix` — a real one between
                    // them would land in the enclosing construct instead.
                    parts.push(
                        if self.comment_has_newline_between(prev_end, comment.span.start) {
                            self.build_trailing_comment_doc_own_line(comment)
                        } else {
                            d.line_suffix(d.concat(&[d.text(" "), self.build_comment_doc(comment)]))
                        },
                    );
                    // A trailing LINE comment must end its own line, so force the group the
                    // run flushes in open; a deferred trailing BLOCK rides `line_suffix`
                    // alone (it flushes before the statement's own terminator/newline) and
                    // must NOT force a break — at an inline value position that would split
                    // the value onto its own line.
                    has_line_comment |= !comment.is_block;
                    prev_end = comment.span.end;
                }
                has_line_comment
            }
        };
        if needs_break {
            // Flush-SCOPED (`DocArena::flush_break`), not the unscoped `break_parent`.
            // The force is redundant for these callers in the first place — every one is
            // a hang seam whose leading comment already emits real hardlines — but
            // "redundant" is not "inert": `arena_fits` returns false the moment its walk
            // reaches a `BreakParent`, and that walk continues into the REST commands, so
            // an unscoped node here also collapses the flat measurement of every SIBLING
            // group before it. The hung value's own group is exactly such a sibling — a
            // conditional constraint printed `(A extends B⏎↹? C⏎↹: D) // t` where its
            // comment-free twin, and prettier, print it flat. The scoped node arms a
            // pending flush instead: the group owning the next line opportunity breaks,
            // one with none stays flat.
            parts.push(d.flush_break());
        }
        d.concat(&parts)
    }

    /// Convenience over [`Self::with_stripped_paren_trailing`] for the common hang site:
    /// build `inner`'s type doc and append any trailing comment lifted from a stripped
    /// `original` shell in one call, so callers don't repeat `inner`. `original` /
    /// `inner` are the seam's `(shell, unwrapped)` pair — equal when nothing was
    /// stripped, a no-op then. `trailing_block` follows
    /// [`Self::with_stripped_paren_trailing`]. The prefix-operator site keeps calling
    /// the lower-level helper directly because it re-parenthesizes the operand first.
    pub(in crate::printer) fn build_hang_value_doc(
        &self,
        original: &TSType<'_>,
        inner: &TSType<'_>,
        trailing_block: TrailingBlock,
    ) -> DocId {
        self.with_stripped_paren_trailing(
            self.build_type_doc(inner),
            original,
            inner,
            trailing_block,
        )
    }

    /// Resolve a keyword→value head: the freeze verdict and the value window, together.
    /// The `as`/`satisfies` keyword, the predicate `is`, the mapped-type `]:` value and
    /// the alias `=` all face the same two-way choice —
    ///
    /// - **frozen** (an alone-on-line directive in the gap, `single_child_frozen`): the
    ///   window is the child's OWN start, deliberately UNWIDENED, so an in-shell
    ///   directive stays on the ordinary paths (`paren_interior_routed_inner` is the
    ///   seam that honors those), and the value is the verbatim slice;
    /// - **not frozen**: the shared paren-strip hang seam
    ///   ([`Self::keyword_value_stripped_paren_hang`]) picks the window and the value.
    ///
    /// One resolver because the two answers must agree: `value_start` is both the gate's
    /// window end and the emitter's claim end, and a head that derived them separately
    /// could gate on one window while claiming another — a dropped or double-printed
    /// comment. [`Self::build_keyword_value_doc`] is the matching value builder.
    /// [`Self::keyword_value_head`] for a **required-pair** position — one where the
    /// author's shell and a pair the construct needs anyway are the same pair (the
    /// type-parameter `extends` constraint; the prefix operator makes the same call
    /// inline). A shell the trailing-run rule RETAINS is left whole here instead of being
    /// handed to the paren-strip hang seam.
    ///
    /// Two things go wrong when the hang takes it. The pair it strips is one the operand
    /// needs, so the arm downstream has to re-mint it — and for an `infer` constraint,
    /// failing to is output the canonical parser rejects. And the trailing `//` it lifts
    /// out renders through a `hardline` that takes whatever indent it flushes at, one
    /// level deeper than the value→`>` gap where the NEXT pass finds that same comment
    /// once the shell is gone: two authorings, two indents, so tsv's own output was not a
    /// fixed point. Retaining answers both — the shell opens, and every comment in it
    /// stays where it was written.
    ///
    /// A **frozen** head still wins: a directive in the keyword gap freezes the child
    /// verbatim, which is a stronger claim on the same bytes.
    pub(in crate::printer) fn keyword_value_head_required_pair<'t>(
        &self,
        gap_start: u32,
        child: &'t TSType<'t>,
    ) -> KeywordValueHead<'t> {
        if !self.single_child_frozen(gap_start, child) && self.paren_retains_for_trailing_run(child)
        {
            return KeywordValueHead {
                gap_start: Some(gap_start),
                frozen: false,
                value_start: child.span().start,
                child,
                value_type: child,
            };
        }
        self.keyword_value_head(gap_start, child)
    }

    pub(in crate::printer) fn keyword_value_head<'t>(
        &self,
        gap_start: u32,
        child: &'t TSType<'t>,
    ) -> KeywordValueHead<'t> {
        let frozen = self.single_child_frozen(gap_start, child);
        let (value_start, value_type) = if frozen {
            (child.span().start, child)
        } else {
            self.keyword_value_stripped_paren_hang(child)
        };
        KeywordValueHead {
            gap_start: Some(gap_start),
            frozen,
            value_start,
            child,
            value_type,
        }
    }

    /// The value doc for a resolved [`Self::keyword_value_head`]: the frozen verbatim
    /// slice, or the hung value with any comment lifted from a stripped shell's trailing
    /// gap appended ([`Self::build_hang_value_doc`] — `trailing_block` per that seam).
    /// Reads the child off the head, so no caller can pair a head with the wrong node.
    pub(in crate::printer) fn build_keyword_value_doc(
        &self,
        head: &KeywordValueHead<'_>,
        trailing_block: TrailingBlock,
    ) -> DocId {
        if head.frozen {
            self.build_frozen_single_child_doc(head.child)
        } else {
            self.build_hang_value_doc(head.child, head.value_type, trailing_block)
        }
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

    /// Build the `import(<specifier>[, <options>])` call portion of an import type.
    /// Qualifier / type arguments are appended by the caller.
    ///
    /// Every comment gap the arguments open — `import(`→specifier, specifier→options,
    /// last argument→`)` — plus an author blank line between the two arguments, is
    /// answered by [`build_import_args_comment_layout`], the layout this construct shares
    /// with its value-level twin, the dynamic-import expression. Prettier prints the two
    /// identically at every one of those gaps *and* at the width boundary (a trailing
    /// comment breaks the parens in both; a bare over-width specifier hangs off the `=`
    /// in both), so one implementation is the whole rule.
    ///
    /// ⚠️ **The hand-rolled version this replaced dropped comments in all three gaps** —
    /// it took the delimiter reading (`PartitionedComments::new`) for a gap that follows
    /// an ITEM, never emitted the dangling half at all, and scanned neither options gap.
    /// Nothing caught it: the import-type shapes had no fixture, and a comment inside an
    /// import type is rare enough that the corpus carries none.
    ///
    /// What stays here is only the clean-region layout: flat, since prettier keeps a bare
    /// import type on one line however long the specifier is.
    fn build_import_type_call_doc(&self, i: &TSImportType<'_>, paren_close: u32) -> DocId {
        let d = self.d();
        let open = d.text("import(");
        // The gap runs from the end of the `import` KEYWORD, not from a computed `(`:
        // `import("` is a fixed string in the OUTPUT, never in the source, so measuring the
        // paren off the span start (`+ "import(".len()`) lands inside a comment written
        // before it and every scan below then starts past that comment — dropping it
        // outright (`import/* c */('a')`, all payloads, and the first comment of a run).
        // A keyword's own bytes can hold no comment, so its end bounds the region exactly
        // and claims both sides of the `(` for the one emitter that prints this slot.
        let paren_gap_start = i.span.start + "import".len() as u32;
        let arg_end = i.argument.span.end;

        // Leading comments between `import` and the specifier.
        let leading = self.build_paren_leading_value_doc(
            paren_gap_start,
            i.argument.span.start,
            self.build_literal_doc(&i.argument),
        );
        let arg_doc = leading.value;

        // Rule A over the options argument, exactly as the dynamic-import expression
        // applies it to its own: an alone-on-line directive in the specifier→options gap
        // freezes the argument that follows it. The specifier itself needs no such route —
        // it is a string literal, which prints verbatim frozen or not.
        let options_arg = i.options.as_ref().map(|options| ImportOptionsArg {
            doc: self.gap_frozen_span(arg_end, options.span()).map_or_else(
                || self.build_expression_doc(options),
                |frozen| self.build_frozen_arg_doc(options, frozen),
            ),
            start: options.span().start,
            end: options.span().end,
        });

        if let Some(doc) = build_import_args_comment_layout(
            self,
            open,
            &leading,
            arg_end,
            options_arg,
            paren_close,
        ) {
            return doc;
        }

        match options_arg {
            Some(options) => d.concat(&[open, arg_doc, d.text(", "), options.doc, d.text(")")]),
            None => d.concat(&[open, arg_doc, d.text(")")]),
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

    /// Whether `ty` is a `TSParenthesizedType` that [`Self::build_parenthesized_type_unwrap_doc`]
    /// **retains** for a trailing line comment — the shell keeps its parens and opens over
    /// real hardlines, so the value owns its own break.
    ///
    /// The retention is what keeps the comment inside the parens the author wrote it in
    /// rather than deferring it past the closer (see that function). An enclosing layout
    /// reads this to know the value breaks *internally* and should therefore hug its `=`,
    /// exactly as a tuple or type literal does. The single predicate for the retain/strip
    /// question — that function consults this too, so an enclosing layout and the shell's
    /// own emission cannot disagree.
    pub(in crate::printer) fn paren_retains_for_trailing_run(&self, ty: &TSType<'_>) -> bool {
        let TSType::Parenthesized(p) = ty else {
            return false;
        };
        self.paren_shell_retains_for_trailing_run(p)
    }

    /// Build `ty`'s doc and wrap it in the parens an enclosing construct **requires**
    /// around it — unless `ty`'s own doc already prints a pair, in which case the required
    /// one is already on the page and `wrap` is skipped.
    ///
    /// The ENCLOSING-side reading of [`Self::paren_retains_for_trailing_run`]: a shell that
    /// rule retains emits its own `(`…`)`, so a caller adding the required pair on top
    /// minted a SECOND one the author never wrote — `keyof ((⏎↹A extends B ? C : D // c⏎))`
    /// where the comment-free authoring prints `keyof (A extends B ? C : D)`. Four sites
    /// asked the question independently (prefix-operator operand, the shared
    /// `build_type_doc_maybe_parens` default arm, array element, indexed-access object) and
    /// all four were wrong the same way; they share it here instead, so a fifth cannot
    /// drift. The rule the divergence catalog states for the shell's own side is the same
    /// one: *a comment never changes which parens are retained, only where it renders once
    /// they are.*
    ///
    /// `wrap` receives the operand doc and supplies the pair — callers differ in what rides
    /// inside it (a bare pair, an `indent`, a following `[]` suffix), so the shape is the
    /// caller's and only the *decision* is shared.
    ///
    /// The prefix operator needs that decision a **second** time, before this: it is the
    /// only required-pair position that also has a keyword→value hang seam, and the hang
    /// would strip the shell before it ever reached here. Both asks read the one predicate.
    pub(in crate::printer) fn build_required_paren_operand_doc(
        &self,
        ty: &TSType<'_>,
        wrap: impl FnOnce(DocId) -> DocId,
    ) -> DocId {
        let operand_doc = self.build_type_doc(ty);
        if self.paren_retains_for_trailing_run(ty) {
            return operand_doc;
        }
        wrap(operand_doc)
    }

    /// Emit a **required** pair OPEN around its operand, with the shell's own deep
    /// interior gaps rendered inside it: `(⏎↹// c⏎↹T⏎)`.
    ///
    /// The shape [`Self::build_parenthesized_type_unwrap_doc`]'s retain arm produces, and
    /// the one a parenthesized **union** operand already reaches through
    /// [`Self::build_parenthesized_union_doc`] — offered here so the operands that are
    /// not unions can reach it too. Glued instead (`(// c⏎↹T)?`), the `(` sits on the
    /// comment's line and the `)` on the type's, which is a third form neither the union
    /// spelling of the same position nor prettier produces.
    ///
    /// The gaps are the **deep** ones — outermost `(` to fully-unwrapped inner, and back
    /// — so a doubly-nested shell (`((// c⏎T))`) still collapses to the one pair and its
    /// comment, which falls between the two `(`s, is still found.
    pub(in crate::printer) fn build_open_required_paren_doc(&self, shell: &TSType<'_>) -> DocId {
        let d = self.d();
        let inner = unwrap_parenthesized(shell);
        let mut parts: DocBuf = DocBuf::new();
        self.push_paren_shell_leading_run(
            &mut parts,
            shell.span().start + 1,
            inner.span().start,
            true,
        );
        parts.push(self.build_type_doc(inner));
        self.push_trailing_comments_in_range(&mut parts, inner.span().end, shell.span().end - 1);
        d.concat(&[
            d.text("("),
            d.indent_hardline(d.concat(&parts)),
            d.hardline(),
            d.text(")"),
        ])
    }

    /// The [`Self::paren_retains_for_trailing_run`] answer for an already-matched shell.
    fn paren_shell_retains_for_trailing_run(&self, p: &TSParenthesizedType<'_>) -> bool {
        // A **line** comment is the only thing that retains the shell: a trailing run of
        // blocks stays inline and the shell still strips. The question is asked as the
        // line-comment one directly, not as a [`Self::paren_inner_comment_flags`] tuple —
        // a line comment is never owned, so "has a trailing line comment" already implies
        // "has a trailing comment to emit". The leading gap is deliberately NOT consulted:
        // a leading comment takes its own real `hardline` either way, so it neither adds
        // to nor cancels the retention.
        let inner = p.type_annotation.span();
        self.has_line_comments_between(inner.end, p.span.end)
            && !self.type_member_separator_follows(p.span.end)
    }

    /// Whether a union / intersection member separator (`|` / `&`) immediately follows
    /// `pos` in source — looking through trivia and through any `)` closers. A crossed
    /// `)` is usually an enclosing redundant layer, which strips along with the shell
    /// and so cannot separate it from the member break; it can also be a RETAINED
    /// closer (a parenthesized union inside an intersection, `B & (A | (C // c)) & D`),
    /// and the licence is deliberately granted there too: the stripped comment then
    /// flushes inside that retained construct, before its `)`, converging onto the
    /// sanctioned union-fit form (`union_intersection_retained_paren_line_comment`) —
    /// still lossless, still one pass.
    ///
    /// This is the one carve-out from the retain rule above, scoped to exactly its
    /// argument: a separator means a per-member break ends the output line right after
    /// this construct, so a deferred trailing comment flushes where it was written —
    /// lossless, the position carrying no signal (the `union_intersection_parens_line_comment`
    /// form, matching prettier). Where nothing but the statement's own tail follows, the
    /// deferred run would escape past the `;` onto a line the reparse cannot re-break —
    /// non-idempotent — so the shell is retained instead
    /// (`type_suffix_trailing_comment_union_member`). A `|`/`&` after a type occurs only
    /// as a member separator, so the byte answers the structural question directly. The
    /// forced break the strip pairs with is flush-scoped (`DocArena::flush_break`), so
    /// the licence never breaks a group the flush doesn't land in — see
    /// [`Self::build_parenthesized_type_unwrap_doc`]'s trailing arm.
    fn type_member_separator_follows(&self, pos: u32) -> bool {
        matches!(self.byte_after_closers(pos), Some(b'|' | b'&'))
    }

    /// Whether a conditional type's `:` still follows `pos` — the branch-position reading
    /// of the same carve-out [`Self::type_member_separator_follows`] states for members.
    ///
    /// A nested-conditional branch's redundant shell is stripped (the branch prints the
    /// clarity pair it decides on, not the one the author typed), so a trailing line
    /// comment in that shell is deferred. That is lossless exactly while the enclosing
    /// conditional still has its `:` to come: the arm break ends the output line right
    /// after this branch and the run flushes on the branch it was written on. In the
    /// FALSE position nothing but the statement's own tail follows, so the same deferral
    /// carries the `//` past the `;` — onto a line the reparse cannot re-break, which is
    /// non-idempotent — and the shell is retained instead.
    ///
    /// Asked of the SOURCE rather than of a true/false flag because the answer is about
    /// what follows the whole nest: an inner false branch inside an outer TRUE branch
    /// (`A extends B ? (C extends D ? E : (F ? G : H // c)) : I`) still has the outer
    /// `:` to flush against, and the `)`-crossing walk finds it.
    pub(in crate::printer) fn conditional_branch_colon_follows(&self, pos: u32) -> bool {
        matches!(self.byte_after_closers(pos), Some(b':'))
    }

    /// The first significant source byte after `pos`, looking through trivia and through
    /// any `)` closers — the shared scanner behind
    /// [`Self::type_member_separator_follows`] and
    /// [`Self::conditional_branch_colon_follows`]. A crossed `)` is an enclosing layer,
    /// redundant or retained, and either way cannot separate this construct from the
    /// break that follows it. `None` at end of source.
    fn byte_after_closers(&self, pos: u32) -> Option<u8> {
        let bytes = self.source.as_bytes();
        let end = bytes.len();
        let mut i = pos as usize;
        while i < end {
            let b = bytes[i];
            if b.is_ascii_whitespace() || b == b')' {
                i += 1;
                continue;
            }
            if let Some(next) = skip_comment(bytes, i, end) {
                i = next;
                continue;
            }
            return Some(b);
        }
        None
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

    /// The node a type effectively renders as once a redundant paren shell carrying
    /// only *leading* comments is stripped — so the **enclosing gap** owns those
    /// comments rather than the paren.
    ///
    /// Where the shell is redundant (prettier strips it too), its leading comments are
    /// physically in the enclosing construct's own gap — a type-argument list's
    /// `<`→first-argument gap, a function type's `=>`→return gap — and that gap's
    /// emitter is what knows the construct's layout: the list's delimiter-line prefix,
    /// the return's continuation hang. Handing it the **unwrapped** node is what makes
    /// the two agree, because `build_parenthesized_type_unwrap_doc` (and
    /// `build_type_doc_for_type_arg`'s `Parenthesized` arm) emit those same comments —
    /// leaving the shell on would print them twice. Ownership of a gap is one
    /// emitter's, never two ([`comments.md`](../../../../docs/comments.md) hazard 3).
    ///
    /// A shell with **trailing** comments keeps its shell: those sit *after* the type,
    /// a gap the enclosing construct does not emit, so the paren doc must stay their
    /// emitter. Nested shells peel while each is leading-only.
    ///
    /// This is what makes the two authorings of one comment agree — `f() => // c⏎T`
    /// and `f() => (// c⏎T)` reach one fixed point instead of two, which is the
    /// `unformatted_ours_*` variants' whole claim.
    pub(in crate::printer) fn leading_paren_unwrapped<'t>(
        &self,
        ty: &'t TSType<'t>,
    ) -> &'t TSType<'t> {
        match ty {
            TSType::Parenthesized(p) if self.paren_inner_comment_flags(p).1 => ty,
            TSType::Parenthesized(p) => self.leading_paren_unwrapped(p.type_annotation),
            other => other,
        }
    }

    /// Unwrap a parenthesized type, preserving any comments inside the parens.
    ///
    /// Block comments are emitted inline: `(/* c */ a)` → `/* c */ a`
    /// Line comments use `line_suffix` to defer to end of the rendered line, plus
    /// `flush_break` to break exactly the group the deferred run flushes in:
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

        // A **line** comment in the trailing gap keeps its place INSIDE the parens, so
        // the shell is retained and opens rather than being stripped: the comment runs
        // to end of line, so `)` cannot follow it. This is the treatment every other
        // bracketed type region gives its own trailing gap, the value-position paren
        // already gives its own (`const e = (⏎x // c⏎);`), and the already-retained
        // union / intersection shells give theirs — so the question is answered one way.
        // Stripping instead carried the comment out to end-of-line, re-binding it from
        // the parenthesized type to the whole statement and landing it on a line that
        // may already hold one, where the two weld irreversibly; it also emitted a
        // `break_parent` for a break the reparse could not reproduce, the parens being
        // gone (F1). See
        // [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
        // §Comment relocation.
        // The one exception, folded into the predicate: a member a `|`/`&` separator
        // immediately follows, whose per-member break ends the line right after it — the
        // stripped comment still trails the member it was written on, lossless, the
        // carve-out §Comment Position Philosophy names
        // (`union_intersection_parens_line_comment`; the last member has no separator
        // and retains — `type_suffix_trailing_comment_union_member`).
        if self.paren_shell_retains_for_trailing_run(p) {
            let mut inner: DocBuf = DocBuf::new();
            if has_leading {
                self.push_paren_shell_leading_run(&mut inner, paren_open, inner_start, true);
            }
            inner.push(self.build_type_doc(p.type_annotation));
            self.push_trailing_comments_in_range(&mut inner, inner_end, paren_close);
            return d.concat(&[
                d.text("("),
                d.indent_hardline(d.concat(&inner)),
                d.hardline(),
                d.text(")"),
            ]);
        }

        let mut parts: DocBuf = DocBuf::new();
        let mut needs_break = false;

        // Leading comments: between `(` and inner type. A line comment terminates at
        // end-of-line, so it takes a `hardline` rather than `line_suffix` — deferring
        // it would push it past the end of the enclosing construct and can produce
        // invalid output (`[// leading a, b]`).
        if has_leading {
            needs_break |=
                self.push_paren_shell_leading_run(&mut parts, paren_open, inner_start, true);
        }

        parts.push(self.build_type_doc(p.type_annotation));

        // Trailing comments: between inner type and `)`. A block stays inline; a line
        // comment defers to end of line and forces the break.
        //
        // ⚠️ The break is load-bearing where the stripped shell has a SIBLING after it:
        // `(a // c) | b` must break the enclosing union so the comment stays on `a`
        // (`union_intersection_parens_line_comment`) — flat, the deferred comment flushes
        // past `| b` and ends up documenting the whole statement instead of the member it
        // was written on. Where there is no sibling the break escapes to the
        // enclosing assignment and splits it after the `=` for nothing — and that split is
        // NOT reproducible (the reparse has no parens left to re-break it), so it was
        // non-idempotent. That case is absorbed at the assignment, by
        // `value_owns_its_comment_break`, which is where the "does the value actually
        // break?" question already lives.
        //
        // `flush_break`, not `break_parent`: the unscoped break also forced every
        // INTERMEDIATE group — a shell nested one composite deep (`B & (A // c) | C`,
        // the shell ending an intersection inside a union member) broke that
        // intersection too, a break the reparse cannot reproduce once the comment sits
        // in the union's member gap (a 2-pass convergence,
        // `type_suffix_trailing_comment_nested_composite`). The flush-scoped node
        // forces only the group the deferred run actually flushes in: the union (its
        // member separator is the next line opportunity) breaks, the intersection —
        // with no line after the suffix — prints flat, which is both formatters'
        // fixed point.
        if has_trailing {
            needs_break |= self.push_trailing_comments_in_range(&mut parts, inner_end, paren_close);
        }

        if needs_break {
            parts.push(d.flush_break());
        }
        d.concat(&parts)
    }
}
