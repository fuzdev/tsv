// Shared class-printer helpers used by both the class-declaration printer
// (`statements/class.rs`) and the class-expression printer
// (`expressions/functions.rs`).
//
// Class declarations and expressions share their entire heritage layout:
// position computation, `extends`/`implements` rendering, the group-mode
// decision, and the body brace placement. Only the prefix differs —
// decorators plus `declare`/`abstract` for declarations, the `class` keyword
// and anonymous-class comment handling for expressions. Everything from the
// heritage clauses onward lives here so the two printers can't drift.
//
// The header→body `{` gap itself is NOT class-specific — every braced-body
// declaration crosses it — so it lives in `pre_body.rs`; the class header takes
// its resolution (`pre_body_gap`) and supplies only the group-relative separator.

use crate::ast::internal;
use crate::printer::CommentSpacing;
use crate::printer::HeritageKeyword;
use crate::printer::Printer;
use crate::printer::class_expr_has_decorators;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Heritage positions computed once and reused for group-mode detection,
/// heritage-comment extraction, and the header→body boundary.
pub(in crate::printer) struct ClassHeritagePositions {
    /// End of the name/type-params region — where heritage leading comments start.
    pub pre_heritage_end: u32,
    /// Position of the `extends` keyword (not the superclass expression start).
    pub extends_keyword_start: Option<u32>,
    /// Position of the `implements` keyword.
    pub implements_keyword_start: Option<u32>,
    /// Start of the first heritage clause keyword (`extends` or `implements`).
    pub first_heritage_start: Option<u32>,
    /// End of the `extends` clause (superclass end, or its type-args end).
    pub extends_clause_end: Option<u32>,
    /// End of the header (last heritage / type-params / name) before the body.
    pub header_end: u32,
}

/// What sits to the left of a class head's `<T>` gap — the question that picks the gap's
/// rule, asked once by [`Printer::push_class_type_params`].
///
/// The two are different gaps, not two spellings of one: after a name the comments belong
/// to the name→`<T>` seam and stay flush (`class A /* c */ <T>`, pinned by
/// `name_type_params_line_comment_prettier_divergence`); with no name the gap belongs to
/// the `class` keyword, which makes it a declaration **header** gap and gives it the
/// uniform forced-continuation indent (`const a = class // c⏎\t<T> {}`).
pub(in crate::printer) enum ClassTypeParamsGap {
    /// The class name's end (`class A /* c */ <T>`).
    Name(u32),
    /// **Any offset within the `class` keyword's own bytes** — its start or its end — for
    /// an anonymous class (`class /* c */ <T>`). The keyword can hold no comment, so both
    /// bound the gap identically, and each printer passes whichever it already has: the
    /// expression printer its located keyword start, the declaration printer its
    /// modifier-walk cursor (already past the keyword). Neither has to derive the other,
    /// which is the point — a keyword's length is arithmetic, and arithmetic over a token
    /// the source may have a comment beside it is what drops comments elsewhere in this
    /// family.
    Keyword(u32),
}

/// How `build_class_header_doc` lays out the class heritage.
///
/// Encodes the real 3-state decision that `(group_mode, has_heritage_line_comments)`
/// spelled as two booleans — the `(non-group, break)` combination is meaningless
/// (a non-group header never consults the break flag), so an enum makes it
/// unrepresentable.
pub(in crate::printer) enum ClassHeaderLayout {
    /// Heritage stays inline; type params break independently (no unified group).
    Independent,
    /// One unified group — heritage breaks with the header on overflow.
    Group,
    /// A unified group forced open (a heritage line comment consumes its line).
    GroupBreak,
}

impl ClassHeaderLayout {
    /// Resolve the layout from the group / heritage-line-comment flags.
    /// `has_heritage_line_comments` is only meaningful in group mode.
    fn from_flags(group_mode: bool, has_heritage_line_comments: bool) -> Self {
        match (group_mode, has_heritage_line_comments) {
            (false, _) => Self::Independent,
            (true, false) => Self::Group,
            (true, true) => Self::GroupBreak,
        }
    }

    /// Whether the heritage sits in one unified group — the `group_mode` flag
    /// [`Printer::build_class_implements_doc`] takes separately, recovered from the
    /// layout so the two can't disagree.
    pub(in crate::printer) fn is_group(&self) -> bool {
        !matches!(self, Self::Independent)
    }
}

/// Non-content inputs to `build_class_header_doc` — the body's shape/position and the
/// heritage layout.
pub(in crate::printer) struct ClassHeaderOptions {
    /// The class body is `{}` — keep ` {}` on the heritage line (never `line()`).
    pub body_is_empty: bool,
    /// Start of the class body `{` — bounds the header→body comment scan.
    pub body_start: u32,
    /// How the heritage clauses lay out (group / independent / forced break).
    pub layout: ClassHeaderLayout,
    /// An own-line directive in the header→`{` gap freezes the body, so the gap's comment
    /// run keeps its own line rather than being spaced onto the header's. Resolved once per
    /// class by the caller (which also emits the frozen body) — see
    /// [`Printer::build_class_body_doc`].
    pub body_frozen: bool,
}

impl<'a> Printer<'a> {
    /// Compute the heritage positions shared by both class printers.
    pub(in crate::printer) fn class_heritage_positions(
        &self,
        span_start: u32,
        id: Option<&internal::Identifier<'_>>,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        super_class: Option<&internal::Expression<'_>>,
        super_type_parameters: Option<&internal::TSTypeParameterInstantiation<'_>>,
        implements: &[internal::TSInterfaceHeritage<'_>],
    ) -> ClassHeritagePositions {
        let pre_heritage_end = type_parameters.map_or_else(
            || id.map_or(span_start + "class".len() as u32, |id| id.span.end),
            |tp| tp.span.end,
        );
        // Find `extends`/`implements` keyword positions (not expression starts) so
        // heritage leading comments only cover name-to-keyword, not keyword-to-item.
        let extends_keyword_start = super_class.and_then(|sc| {
            self.find_keyword_in_range(pre_heritage_end, sc.span().start, "extends")
        });
        let implements_keyword_start = if implements.is_empty() {
            None
        } else {
            let search_start = extends_keyword_start.map_or(pre_heritage_end, |ek| {
                // After the extends clause.
                super_class.map_or(ek + "extends".len() as u32, |sc| {
                    super_type_parameters.map_or_else(|| sc.span().end, |tp| tp.span.end)
                })
            });
            self.find_keyword_in_range(search_start, implements[0].span.start, "implements")
        };
        let first_heritage_start = extends_keyword_start.or(implements_keyword_start);
        let extends_clause_end = super_class
            .map(|sc| super_type_parameters.map_or_else(|| sc.span().end, |tp| tp.span.end));
        let header_end = implements
            .last()
            .map(|i| i.span.end)
            .or(extends_clause_end)
            .or_else(|| type_parameters.map(|tp| tp.span.end))
            .or_else(|| id.map(|id| id.span.end))
            .unwrap_or(span_start + "class".len() as u32);

        ClassHeritagePositions {
            pre_heritage_end,
            extends_keyword_start,
            implements_keyword_start,
            first_heritage_start,
            extends_clause_end,
            header_end,
        }
    }

    /// Resolve how a class header lays its heritage out — the whole of the decision,
    /// shared by both class printers, over the positions
    /// [`Self::class_heritage_positions`] already computed.
    ///
    /// Group mode is the structural rule ([`Self::should_class_group_mode`]) OR a
    /// comment **on the page** in either heritage gap: name/type-params→first keyword,
    /// and the extends-clause→`implements` gap. A *line* comment in either gap forces
    /// the group open, because it consumes the rest of its line — as
    /// [`ClassHeaderLayout::GroupBreak`] rather than a `break_parent`, so the break
    /// doesn't pollute `fits()` lookahead for nested groups (a `<T>` list would break
    /// with it).
    pub(in crate::printer) fn class_header_layout(
        &self,
        positions: &ClassHeritagePositions,
        super_class: Option<&internal::Expression<'_>>,
        super_type_parameters: Option<&internal::TSTypeParameterInstantiation<'_>>,
        implements: &[internal::TSInterfaceHeritage<'_>],
    ) -> ClassHeaderLayout {
        // The extends→`implements` gap exists only with both clauses present, which is
        // already `count > 1` in the structural rule — so that arm can only ever add to
        // the *line* half below, never to `group_mode`.
        let extends_to_implements = positions
            .extends_clause_end
            .filter(|_| !implements.is_empty())
            .map(|ext_end| (ext_end, implements[0].span.start));
        let has_heritage_comments = positions
            .first_heritage_start
            .is_some_and(|hs| self.has_comments_on_page_between(positions.pre_heritage_end, hs))
            || extends_to_implements.is_some_and(|(a, b)| self.has_comments_on_page_between(a, b));
        let group_mode =
            self.should_class_group_mode(super_class, super_type_parameters, implements)
                || has_heritage_comments;
        let has_heritage_line_comments = positions
            .first_heritage_start
            .is_some_and(|hs| self.has_line_comments_between(positions.pre_heritage_end, hs))
            || extends_to_implements.is_some_and(|(a, b)| self.has_line_comments_between(a, b));
        ClassHeaderLayout::from_flags(group_mode, has_heritage_line_comments)
    }

    /// Emit a class head's `<T>` type-parameter list, preceded by the gap before it.
    ///
    /// Both arms emit the gap — [`ClassTypeParamsGap`] names which rule it takes, keyed on
    /// what sits to its left, so neither arm can be the one nobody claims.
    ///
    /// The list itself always uses the wrapping builder, so it breaks on width
    /// independently of the heritage group.
    pub(in crate::printer) fn push_class_type_params(
        &self,
        parts: &mut DocBuf,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        gap: ClassTypeParamsGap,
    ) {
        let Some(type_params) = type_parameters else {
            return;
        };
        let list_start = type_params.span.start;
        match gap {
            ClassTypeParamsGap::Name(name_end) => {
                // A line comment takes a hardline here, so it can't absorb the `<T>` as
                // comment text.
                self.push_name_to_type_params_comments(
                    parts,
                    name_end,
                    list_start,
                    CommentSpacing::Trailing,
                );
                parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }
            ClassTypeParamsGap::Keyword(keyword_start) => {
                let list_doc = self.build_type_parameter_declaration_doc_wrapping(type_params);
                if !self.has_comments_to_emit_between(keyword_start, list_start) {
                    // The overwhelmingly common case: `class<T>` hugs the keyword, so the
                    // gap contributes nothing — not even a separator.
                    parts.push(list_doc);
                    return;
                }
                let d = self.d();
                let run = self.build_name_to_type_params_comments(
                    keyword_start,
                    list_start,
                    CommentSpacing::Trailing,
                );
                let body = d.concat(&[run, list_doc]);
                // A line comment ends the gap's line, so `<T>` starts a continuation line:
                // indent it one level, the uniform declaration-header rule every other
                // keyword gap takes. The indent must cover the run, whose hardline is the
                // break being indented.
                parts.push(if self.keyword_gap_breaks(keyword_start, list_start) {
                    d.indent(body)
                } else {
                    body
                });
            }
        }
    }

    /// Whether a class should use heritage "group mode" for structural
    /// (non-comment) reasons:
    /// 1. Multiple heritage items (extends + implements count > 1), or
    /// 2. A member-expression superclass without type arguments.
    ///
    /// Comment-based group mode is OR'd in by each caller using the already
    /// computed position data (avoids duplicate binary searches).
    pub(in crate::printer) fn should_class_group_mode(
        &self,
        super_class: Option<&internal::Expression<'_>>,
        super_type_parameters: Option<&internal::TSTypeParameterInstantiation<'_>>,
        implements: &[internal::TSInterfaceHeritage<'_>],
    ) -> bool {
        let mut count = if super_class.is_some() { 1 } else { 0 };
        count += implements.len();
        if count > 1 {
            return true;
        }
        if let Some(super_class) = super_class
            && super_type_parameters.is_none()
            && matches!(super_class, internal::Expression::MemberExpression(_))
        {
            return true;
        }
        false
    }

    /// `extends <super_class>[<type args>]`, preserving comments between the
    /// `extends` keyword and the base (`extends /* c */ Base`) and between the
    /// base and its type args (`extends Base/* c */ <T>`).
    ///
    /// The superclass is rendered with the full expression printer so it breaks
    /// width-aware and keeps inner comments; the type args use the wrapping
    /// type-argument builder (the same one the `implements` clause uses).
    pub(in crate::printer) fn build_class_extends_doc(
        &self,
        super_class: Option<&internal::Expression<'_>>,
        super_type_parameters: Option<&internal::TSTypeParameterInstantiation<'_>>,
        extends_keyword_start: Option<u32>,
    ) -> Option<DocId> {
        let d = self.d();
        let super_class = super_class?;
        // A line comment or multiline block after `extends`, before the super class,
        // hangs the super class on the next line — the shared as/satisfies + type-param
        // keyword→value mechanism. A single-line block comment (own-line, trailing, or
        // glued) collapses inline (`extends /* c */ B`, the fall-through below);
        // prettier relocates the collapsed comment before `extends`.
        if let Some(kw_start) = extends_keyword_start {
            let kw_end = kw_start + "extends".len() as u32;
            if self.comments_force_own_line_between(kw_end, super_class.span().start) {
                let mut value_parts: DocBuf = smallvec![self.build_super_class_doc(super_class)];
                if let Some(type_args) = super_type_parameters {
                    let gap_start = super_class.span().end;
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
                let mut ext_parts = smallvec![d.text("extends")];
                self.append_keyword_value_line_comments(
                    &mut ext_parts,
                    kw_end,
                    super_class.span().start,
                    value_doc,
                );
                return Some(d.concat(&ext_parts));
            }
        }
        let mut ext_parts: DocBuf = smallvec![d.text("extends ")];
        if let Some(kw_start) = extends_keyword_start {
            let kw_end = kw_start + "extends".len() as u32;
            if let Some(comments) = self.build_inline_comments_between_doc_trailing_space_opt(
                kw_end,
                super_class.span().start,
            ) {
                ext_parts.push(comments);
            }
        }
        ext_parts.push(self.build_super_class_doc(super_class));
        if let Some(type_args) = super_type_parameters {
            let gap_start = super_class.span().end;
            let gap_end = type_args.span.start;
            if let Some(doc) = self.build_name_to_type_params_comments_opt(
                gap_start,
                gap_end,
                CommentSpacing::Trailing,
            ) {
                ext_parts.push(doc);
            }
            ext_parts.push(self.build_type_arguments_doc(type_args));
        }
        Some(d.concat(&ext_parts))
    }

    /// Render the superclass expression, wrapping it in parens when prettier's
    /// heritage rule requires it (`extends (a + b)`, `extends (new X())`,
    /// `extends ((a) => b)`, …). A bare `Base<T>` keeps `Base` unwrapped — its type
    /// arguments are rendered separately via `super_type_parameters`.
    fn build_super_class_doc(&self, super_class: &internal::Expression<'_>) -> DocId {
        // A continuation-indent position; the parens below are the PRINTER's and stay
        // outside the operand's own doc.
        let doc = self.build_continuation_indent_expression_doc(super_class);
        if self.needs_parens(super_class, super::ParenContext::SuperClass) {
            // A decorated class expression breaks its parens open and indents the
            // content (prettier), the decorators forcing the break:
            // `extends (⏎\t@deco⏎\tclass {}⏎)`. Every other wrapped heritage form
            // stays inline in flat parens.
            if matches!(
                super_class,
                internal::Expression::ClassExpression(c) if class_expr_has_decorators(c)
            ) {
                self.build_break_open_parens(doc)
            } else {
                self.d().parens(doc)
            }
        } else {
            doc
        }
    }

    /// `implements <items>`, delegating to the shared heritage-clause builder
    /// (normalizes whitespace, breaks long lists per item, renders type args
    /// and inter-item comments).
    pub(in crate::printer) fn build_class_implements_doc(
        &self,
        implements: &[internal::TSInterfaceHeritage<'_>],
        group_mode: bool,
        implements_keyword_start: Option<u32>,
    ) -> Option<DocId> {
        if implements.is_empty() {
            return None;
        }
        Some(self.build_heritage_clause_doc(
            HeritageKeyword::Implements,
            implements,
            group_mode,
            implements_keyword_start,
        ))
    }

    /// Assemble the class header and wrap it in the header group.
    ///
    /// `parts` already holds the prefix (keyword/name/type params). This appends
    /// the heritage clauses and the pre-body brace spacing, then returns the
    /// group-wrapped header. The body is appended by the caller OUTSIDE this
    /// group so the body's hardlines don't pollute the header's fit check.
    ///
    /// The header→body `{` gap is resolved here for **every** class shape, declaration and
    /// expression alike — bare name, anonymous, heritage, type params. Gating it
    /// behind a "the caller emits this gap itself" flag a class-expression printer sets
    /// false for its bare-name / anonymous forms has those two routes answer it
    /// differently from their own siblings (collapsing a broke-after multiline
    /// block, and emitting a stray space before the brace). A flag meaning "another emitter
    /// already claimed this range" is the `docs/comments.md` §3 hazard, not a knob.
    pub(in crate::printer) fn build_class_header_doc(
        &self,
        mut parts: DocBuf,
        positions: &ClassHeritagePositions,
        extends_doc: Option<DocId>,
        implements_doc: Option<DocId>,
        implements: &[internal::TSInterfaceHeritage<'_>],
        options: ClassHeaderOptions,
    ) -> DocId {
        let d = self.d();
        let header_end = positions.header_end;

        if matches!(options.layout, ClassHeaderLayout::Independent) {
            // Non-group mode: heritage stays inline, type params break independently.
            if let Some(ext) = extends_doc {
                parts.push(d.text(" "));
                parts.push(ext);
            }
            if let Some(impl_doc) = implements_doc {
                parts.push(d.text(" "));
                parts.push(impl_doc);
            }
            parts.push(self.build_header_pre_body_doc(
                header_end,
                options.body_start,
                options.body_frozen,
            ));
            return d.concat(&parts);
        }

        // Group mode: one unified group — when it breaks, heritage breaks too.
        // Comments between name/type-params and the first heritage clause.
        let mut extra_heritage_comments = DocBuf::new();
        if let Some(heritage_start) = positions.first_heritage_start {
            let (inline, indent) = self
                .build_heritage_leading_comment_parts(positions.pre_heritage_end, heritage_start);
            parts.extend(inline);
            extra_heritage_comments = indent;
        }

        let mut heritage_parts = extra_heritage_comments;
        if let Some(ext) = extends_doc {
            heritage_parts.push(d.line());
            heritage_parts.push(ext);
            // Comments between the extends clause and the implements keyword.
            // Use implements_keyword_start to avoid double-counting keyword comments.
            if let Some(ext_end) = positions.extends_clause_end
                && !implements.is_empty()
            {
                let mid_end = positions
                    .implements_keyword_start
                    .unwrap_or(implements[0].span.start);
                if let Some(mid_comments) =
                    self.build_inline_comments_between_doc_opt(ext_end, mid_end)
                {
                    heritage_parts.push(mid_comments);
                }
            }
        }
        if let Some(impl_doc) = implements_doc {
            heritage_parts.push(d.line());
            heritage_parts.push(impl_doc);
        }
        if !heritage_parts.is_empty() {
            parts.push(d.indent(d.concat(&heritage_parts)));
        }

        // Comments between header and body, plus the pre-brace spacing — the same gap
        // resolution every other braced-body declaration takes, differing only in the
        // separator this group wants: for a non-empty body `line()` puts the brace on its
        // own line when the group breaks, while an empty body always keeps ` {}` on the
        // heritage line.
        let (gap_comments, gap_breaks) =
            self.pre_body_gap(header_end, options.body_start, options.body_frozen);
        if let Some(comments) = gap_comments {
            parts.push(comments);
        }
        if gap_breaks {
            parts.push(d.hardline());
        } else if options.body_is_empty {
            parts.push(d.text(" "));
        } else {
            parts.push(d.line());
        }

        let parts_doc = d.concat(&parts);
        if matches!(options.layout, ClassHeaderLayout::GroupBreak) {
            d.group_break(parts_doc)
        } else {
            d.group(parts_doc)
        }
    }
}
