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

use crate::ast::internal;
use crate::printer::CommentSpacing;
use crate::printer::HeritageKeyword;
use crate::printer::Printer;
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
                    value_parts.push(self.build_type_arguments_doc_wrapping(type_args));
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
            ext_parts.push(self.build_comments_between(
                kw_end,
                super_class.span().start,
                CommentSpacing::Trailing,
            ));
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
            ext_parts.push(self.build_type_arguments_doc_wrapping(type_args));
        }
        Some(d.concat(&ext_parts))
    }

    /// Render the superclass expression, wrapping it in parens when prettier's
    /// heritage rule requires it (`extends (a + b)`, `extends (new X())`,
    /// `extends ((a) => b)`, …). A bare `Base<T>` keeps `Base` unwrapped — its type
    /// arguments are rendered separately via `super_type_parameters`.
    fn build_super_class_doc(&self, super_class: &internal::Expression<'_>) -> DocId {
        let doc = self.build_expression_doc(super_class);
        if self.needs_parens(super_class, super::ParenContext::SuperClass) {
            // A decorated class expression breaks its parens open and indents the
            // content (prettier), the decorators forcing the break:
            // `extends (⏎\t@deco⏎\tclass {}⏎)`. Every other wrapped heritage form
            // stays inline in flat parens.
            if matches!(
                super_class,
                internal::Expression::ClassExpression(c)
                    if c.decorators.is_some_and(|dec| !dec.is_empty())
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

    /// Spacing (and any comment) between a class header and its body brace:
    /// `extends B /* c */ {}`, `class A<T> /* c */ {}`. Line comments force a
    /// hardline (they'd absorb the brace); block comments keep a space.
    ///
    /// `emit_comments` gates the comment scan: the class-expression printer's
    /// bare name→body / anonymous→body paths emit their own comments, so it
    /// passes `false` when there is no heritage or type params.
    /// Emit the comments between a class/interface header (after the last
    /// heritage item or type params) and the body `{`, plus the pre-`{` spacing.
    /// Shared by the class-declaration non-group path and the interface printer.
    /// Comments are preserved each on their own line via `build_pre_body_comments_doc`
    /// (line comments don't absorb following comments); a line comment forces the
    /// brace onto the next line, otherwise it hugs with a single space. Returns a
    /// bare `" "` when there are no comments (or `emit_comments` is false).
    pub(in crate::printer) fn build_header_pre_body_doc(
        &self,
        emit_comments: bool,
        header_end: u32,
        body_start: u32,
    ) -> DocId {
        let d = self.d();
        if emit_comments
            && let Some(comments) = self.build_pre_body_comments_doc(header_end, body_start)
        {
            if self.has_line_comments_between(header_end, body_start) {
                d.concat(&[comments, d.hardline()])
            } else {
                d.concat(&[comments, d.text(" ")])
            }
        } else {
            d.text(" ")
        }
    }

    /// Assemble the class header and wrap it in the header group.
    ///
    /// `parts` already holds the prefix (keyword/name/type params). This appends
    /// the heritage clauses and the pre-body brace spacing, then returns the
    /// group-wrapped header. The body is appended by the caller OUTSIDE this
    /// group so the body's hardlines don't pollute the header's fit check.
    ///
    /// `emit_pre_body_comments` gates the header→body comment scan: the
    /// class-expression printer's bare name→body / anonymous→body paths emit
    /// their own comments, so it passes `false` when there is no heritage or
    /// type params (the declaration always passes `true`).
    #[allow(clippy::too_many_arguments)]
    pub(in crate::printer) fn build_class_header_doc(
        &self,
        mut parts: DocBuf,
        positions: &ClassHeritagePositions,
        extends_doc: Option<DocId>,
        implements_doc: Option<DocId>,
        implements: &[internal::TSInterfaceHeritage<'_>],
        body_is_empty: bool,
        body_start: u32,
        group_mode: bool,
        has_heritage_line_comments: bool,
        emit_pre_body_comments: bool,
    ) -> DocId {
        let d = self.d();
        let header_end = positions.header_end;

        if !group_mode {
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
                emit_pre_body_comments,
                header_end,
                body_start,
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

        // Comments between header and body, plus the pre-brace spacing.
        // Line comments force a hardline (they'd absorb the brace). For a
        // non-empty body, `line()` puts the brace on its own line when the
        // group breaks; an empty body always keeps ` {}` on the heritage line.
        let has_line_comment =
            emit_pre_body_comments && self.has_line_comments_between(header_end, body_start);
        if emit_pre_body_comments
            && let Some(comments) = self.build_pre_body_comments_doc(header_end, body_start)
        {
            parts.push(comments);
        }
        if has_line_comment {
            parts.push(d.hardline());
        } else if body_is_empty {
            parts.push(d.text(" "));
        } else {
            parts.push(d.line());
        }

        let parts_doc = d.concat(&parts);
        if has_heritage_line_comments {
            d.group_break(parts_doc)
        } else {
            d.group(parts_doc)
        }
    }
}
