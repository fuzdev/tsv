// Class declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use crate::printer::class_common::ClassHeaderOptions;
use crate::printer::class_common::ClassTypeParamsGap;
use crate::printer::expressions::assignment::{AssignmentLeft, RhsCommentInfo};
use crate::printer::{
    ClassMemberModifiers, ContinuationValue, MemberBlankScan, MemberBody, MemberFloor,
    MemberFreeze, MemberSeam,
};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

/// What a class field's value needs to know about its own position, resolved once by
/// [`Printer::property_value_facts`] and handed to both paths that print that value.
#[derive(Clone, Copy)]
struct PropertyValueFacts {
    /// The `=`→value head's freeze ([`Printer::value_head_frozen_span`]): the span to emit
    /// verbatim in place of the value's doc.
    frozen: Option<Span>,
    /// Whether the position supplies clarity parens ([`Printer::needs_parens`] at
    /// `ParenContext::DefaultValue`).
    position_parens: bool,
}

impl<'a> Printer<'a> {
    /// Build a Doc for a class declaration
    #[inline]
    pub(super) fn build_class_declaration_doc(
        &self,
        decl: &internal::ClassDeclaration<'_>,
    ) -> DocId {
        self.build_class_declaration_doc_inner(decl, true)
    }

    /// Build a Doc for a class declaration without decorators
    ///
    /// Used when exporting decorated classes where decorators are printed
    /// before the export keyword.
    #[inline]
    pub(in crate::printer) fn build_class_declaration_without_decorators_doc(
        &self,
        decl: &internal::ClassDeclaration<'_>,
    ) -> DocId {
        self.build_class_declaration_doc_inner(decl, false)
    }

    /// The source position where a class declaration's own doc begins: its first
    /// keyword (`declare` / `abstract` / `class`), located past any decorators.
    ///
    /// A caller that prints the decorators itself and then the *undecorated* class
    /// (the decorators-first `export default` path) needs this to bound its own
    /// keyword→value gap. Without it that gap has no end, so nothing scans it and a
    /// comment authored there is dropped.
    pub(in crate::printer) fn class_declaration_keyword_start(
        &self,
        decl: &internal::ClassDeclaration<'_>,
    ) -> u32 {
        let first_keyword = if decl.declare {
            "declare"
        } else if decl.r#abstract {
            "abstract"
        } else {
            "class"
        };
        self.find_keyword_after_decorators(decl.decorators, first_keyword, decl.span.start)
    }

    /// Core implementation for class declaration doc building
    ///
    /// # Arguments
    ///
    /// * `decl` - The class declaration to build a doc for
    /// * `include_decorators` - If true, decorators are included in the output.
    ///   Set to false when decorators are printed separately (e.g., before `export`).
    fn build_class_declaration_doc_inner(
        &self,
        decl: &internal::ClassDeclaration<'_>,
        include_decorators: bool,
    ) -> DocId {
        let d = self.d();

        // Compute heritage positions once (shared with the class-expression printer).
        let positions = self.class_heritage_positions(
            decl.span.start,
            decl.id.as_ref(),
            decl.type_parameters.as_ref(),
            decl.super_class,
            decl.super_type_parameters.as_ref(),
            decl.implements,
        );

        // Heritage layout (shared with the class-expression printer).
        let layout = self.class_header_layout(
            &positions,
            decl.super_class,
            decl.super_type_parameters.as_ref(),
            decl.implements,
        );

        let mut parts = smallvec![];

        // Decorators, each on its own line; the first keyword after them
        // (declare/abstract/class) is where this class's own text starts.
        let keyword_start = self.class_declaration_keyword_start(decl);

        if include_decorators
            && let Some(dec_doc) = self.build_decorators_doc(decl.decorators, keyword_start)
        {
            parts.push(dec_doc);
        }

        // Emit modifiers with comments preserved between each keyword pair
        // e.g., `abstract/* b */class B` → `abstract /* b */ class B`
        let search_end = decl.id.as_ref().map_or(decl.span.end, |id| id.span.start);
        let mut cursor = keyword_start;

        if decl.declare {
            parts.push(d.text("declare"));
            cursor = keyword_start + "declare".len() as u32;
        }
        if decl.r#abstract {
            // Find "abstract" in source after cursor, skipping comments
            let abstract_pos = self.find_keyword_in_range(cursor, search_end, "abstract");
            if let Some(ap) = abstract_pos {
                if let Some(c) = self.build_inline_comments_between_doc_opt(cursor, ap) {
                    parts.push(c);
                }
                if cursor > keyword_start {
                    parts.push(d.text(" "));
                }
                parts.push(d.text("abstract"));
                cursor = ap + "abstract".len() as u32;
            }
        }
        // Find "class" in source after cursor, skipping comments
        let class_pos = self.find_keyword_in_range(cursor, search_end, "class");
        if let Some(cp) = class_pos {
            if let Some(c) = self.build_inline_comments_between_doc_opt(cursor, cp) {
                parts.push(c);
            }
            if cursor > keyword_start {
                parts.push(d.text(" "));
            }
            parts.push(d.text("class"));
            cursor = cp + "class".len() as u32;
        }

        // Build heritage docs (shared with the class-expression printer).
        let extends_doc = self.build_class_extends_doc(
            decl.super_class,
            decl.super_type_parameters.as_ref(),
            positions.extends_keyword_start,
        );
        let implements_doc = self.build_class_implements_doc(
            decl.implements,
            layout.is_group(),
            positions.implements_keyword_start,
        );

        // The header→`{` gap's freeze, resolved once: the header builder needs the placement
        // answer (keep the run's own line) and the body builder needs the span.
        let frozen_body = self.gap_frozen_span(positions.header_end, decl.body.span);

        // The named and anonymous paths differ only in the header `parts` they hand the
        // builder, so the options — including the freeze verdict above — are resolved once
        // here. Carries no `DocId`, so hoisting it past the header doc is arena-neutral.
        let header_options = ClassHeaderOptions {
            body_is_empty: decl.body.body.is_empty(),
            body_start: decl.body.span.start,
            layout,
            body_frozen: frozen_body.is_some(),
        };

        if let Some(id) = &decl.id {
            // Named: collect the name + type params + heritage + body into one
            // continuation so a *line* comment in the `class`→name gap indents the
            // whole declaration one level (uniform declaration-header rule). Block
            // and no-comment cases stay inline.
            let mut header_parts = smallvec![self.identifier_name_doc(id)];
            self.push_class_type_params(
                &mut header_parts,
                decl.type_parameters.as_ref(),
                ClassTypeParamsGap::Name(id.span.end),
            );
            let header_doc = self.build_class_header_doc(
                header_parts,
                &positions,
                extends_doc,
                implements_doc,
                decl.implements,
                header_options,
            );
            let continuation = d.concat(&[
                header_doc,
                self.build_class_body_doc(&decl.body, frozen_body),
            ]);
            parts.push(self.build_keyword_to_name_continuation(
                cursor,
                id.span.start,
                continuation,
            ));
            return d.concat(&parts);
        }

        // Anonymous class declaration (`export default class {}`): the keyword→body
        // / →heritage gap is handled by the header builder, unchanged. The keyword→`<T>`
        // gap is this printer's own — `cursor` is the `class` keyword, found rather than
        // measured off the span.
        self.push_class_type_params(
            &mut parts,
            decl.type_parameters.as_ref(),
            ClassTypeParamsGap::Keyword(cursor),
        );
        let header_doc = self.build_class_header_doc(
            parts,
            &positions,
            extends_doc,
            implements_doc,
            decl.implements,
            header_options,
        );

        d.concat(&[
            header_doc,
            self.build_class_body_doc(&decl.body, frozen_body),
        ])
    }

    /// The slot floor for a class-member gap: past the LAST stray `;` between the
    /// members, else the previous member's end. A stray `;` in a class body produces
    /// no member node yet still closes its slot — a comment before it trails the
    /// previous member however the lines fall (`a = 1; /* c */ ; b = 2;` — prettier
    /// binds it the same way), so the leads-next scan
    /// ([`Printer::trailing_claim_end`]) must not reach back across one. The
    /// statement lists get the same rule from their dropped `EmptyStatement` nodes
    /// ([`statement_gap_floor`](crate::printer::statement_gap_floor)); a class body
    /// has no node to key on, so the floor reads the source.
    pub(in crate::printer) fn class_member_gap_floor(
        &self,
        member_end: u32,
        next_start: u32,
    ) -> u32 {
        let mut floor = member_end;
        while let Some(pos) = self.find_char_outside_comments(floor, next_start, b';') {
            floor = pos + 1;
        }
        floor
    }

    /// Build a Doc for a class body
    ///
    /// Handles comments between members, blank line preservation, and trailing comments.
    /// `frozen` is the header→`{` gap's format-ignore answer, resolved by the caller (which
    /// also passes it to the header builder as `ClassHeaderOptions::body_frozen`, so the
    /// gap's comment run keeps the own line that makes the directive honored). `Some` is the
    /// body's own span, emitted verbatim: the braces and every member ride inside the slice
    /// while the name and heritage stay parent-owned outside it. Prettier instead relocates
    /// the directive into the body and freezes only the first member
    /// (`body_prettier_ignore_head_prettier_divergence`).
    pub(in crate::printer) fn build_class_body_doc(
        &self,
        body: &internal::ClassBody<'_>,
        frozen: Option<Span>,
    ) -> DocId {
        let d = self.d();
        if let Some(frozen) = frozen {
            return self.build_frozen_span_doc(frozen);
        }
        if body.body.is_empty() {
            return self.build_empty_body_with_comments_doc(body.span);
        }

        // A comment trailing the opening `{` on its own line is kept on the `{`
        // line when the body expands (divergence from prettier, which relocates
        // it to its own line as the first member's leading comment). Same
        // mechanism as block/namespace bodies. See conformance_prettier_ts_comments.md
        // §Comment relocation (Class/interface/enum body `{`).
        let first_member_start = body.body[0].span().start;
        let (brace_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(body.span.start, first_member_start);

        // Build member docs with comments and blank line preservation, via the shared
        // member-body walk — the same one the interface body and both type-literal
        // force-multiline walks take. A class body's own facts: its stray `;`s close
        // their slots while producing no member node ([`Printer::class_member_gap_floor`]),
        // its members print their own terminators, and a directive freezes the whole
        // member span.
        //
        // Zero-comment fast gate: one binary search over the class-body span
        // short-circuits every per-member comment sub-query (leading collect,
        // format-ignore lookup, trailing-comment scan, trailing-end walk, and
        // trailing-body comments). Sound because comments are disjoint + start-sorted
        // and every sub-range lies within the body span, so when none sit inside the
        // body all sub-queries are provably empty/false. Blank-line preservation is
        // comment-independent and stays.
        let mut member_parts = d.pooled_docbuf();
        self.build_member_list_docs_into(
            &mut member_parts,
            body.body,
            MemberBody {
                span: body.span,
                has_comments: self.has_comments_on_page_between(body.span.start, body.span.end),
                delimiter_pull_pos,
                blank_scan: MemberBlankScan::FromCursor,
                freeze: MemberFreeze::Span,
                seam: MemberSeam::Whole {
                    floor: MemberFloor::PastStraySemicolons,
                },
            },
            internal::ClassMember::span,
            |member| member.span().end,
            |member, _deferred| self.build_class_member_doc(member),
        );

        // Wrap body content in indent
        self.build_delimited_doc(
            d.text("{"),
            brace_line_prefix,
            d.indent(d.concat(&member_parts)),
            d.hardline(),
            d.text("}"),
        )
    }

    /// Build a Doc for a class member
    fn build_class_member_doc(&self, member: &internal::ClassMember<'_>) -> DocId {
        match member {
            internal::ClassMember::MethodDefinition(method) => {
                self.build_method_definition_doc(method)
            }
            internal::ClassMember::PropertyDefinition(prop) => {
                self.build_property_definition_doc(prop)
            }
            internal::ClassMember::StaticBlock(block) => self.build_static_block_doc(block),
            internal::ClassMember::IndexSignature(sig) => self.build_index_signature_doc(sig),
        }
    }

    /// Build a Doc for a class index signature: `[key: Type]: ValueType;`.
    /// Delegates to the shared `build_index_signature_member_doc` (which handles
    /// the `static`/`readonly` modifiers and every in-bracket comment gap) and
    /// appends the trailing `;`, matching the interface caller. An own-line comment
    /// in the value-type→`;` gap defers past the `;` (prettier).
    fn build_index_signature_doc(&self, sig: &internal::TSIndexSignature<'_>) -> DocId {
        let mut deferred = DocBuf::new();
        let member = self.build_index_signature_member_doc(sig, &mut deferred);
        self.build_member_with_semicolon_doc(member, deferred)
    }

    /// Build a Doc for a static initialization block
    // TODO: `StaticBlock` reuses `BlockStatement`'s doc-building machinery via
    // this synthetic wrapper purely to save duplicating the body-printing logic,
    // but a `StaticBlock` isn't a `BlockStatement` (see `build_static_block_body_doc`
    // and its `in_program_or_block=false` carve-out) — a second such divergent
    // property would need a second bolt-on. Worth a real `StaticBlock`-native path
    // (or an explicit node-kind tag) if that happens.
    fn build_static_block_doc(&self, block: &internal::StaticBlock<'_>) -> DocId {
        let d = self.d();
        // Create a BlockStatement wrapper to reuse existing doc building logic
        let block_stmt = internal::BlockStatement {
            body: block.body,
            span: block.span,
        };
        d.concat(&[
            d.text("static "),
            self.build_static_block_body_doc(&block_stmt),
        ])
    }

    /// Build a Doc for a property definition
    fn build_property_definition_doc(&self, prop: &internal::PropertyDefinition<'_>) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];

        // Decorators (inline or own-line depending on original source)
        let next_token_start = prop
            .decorators
            .as_ref()
            .and_then(|decs| decs.last())
            .map_or(prop.span.start, |dec| {
                self.find_first_token_after(dec.span.end)
            });
        if let Some(dec_doc) =
            self.build_class_member_decorators_doc(prop.decorators, next_token_start)
        {
            parts.push(dec_doc);
        }

        // Modifier keywords, preserving comments between them and before the
        // name (e.g., `static /* c */ readonly p`). `cursor` tracks the scan
        // position so each comment is emitted at the user's placement.
        let key_start = prop.key.span().start;
        let mut cursor = next_token_start;

        // Declare modifier (property-only, and first — ahead of accessibility)
        if prop.declare {
            self.push_member_keyword_doc(&mut parts, "declare ", &mut cursor, key_start);
        }

        // Accessibility → static → abstract → override, the run shared with methods
        self.push_class_member_modifiers_doc(
            &mut parts,
            ClassMemberModifiers {
                accessibility: prop.accessibility,
                is_static: prop.is_static,
                r#abstract: prop.r#abstract,
                r#override: prop.r#override,
            },
            &mut cursor,
            key_start,
        );

        // Readonly modifier
        if prop.readonly {
            self.push_member_keyword_doc(&mut parts, "readonly ", &mut cursor, key_start);
        }

        // Accessor keyword
        if prop.accessor {
            self.push_member_keyword_doc(&mut parts, "accessor ", &mut cursor, key_start);
        }

        // Key (track key_region_end to avoid double-counting comments inside brackets)
        let key_region_end;
        if prop.computed {
            // Comments before the `[` (inside-bracket comments are handled by
            // the bracket builder)
            let bracket_pos = find_char_skipping_comments(
                self.source.as_bytes(),
                cursor as usize,
                key_start as usize,
                b'[',
            )
            .map_or(key_start, |p| p as u32);
            self.push_pre_name_comments_doc(&mut parts, cursor, bracket_pos);
            // Parenthesize an assignment-expression computed key (`[(x = 0)] = 1`)
            // and an `in` key inside a for-header init, exactly like the object
            // computed-key path (shared helper).
            let (doc, end) = self.build_computed_key_bracket_doc(
                cursor,
                &prop.key,
                Some(super::ParenContext::ComputedPropertyKey),
                || self.build_computed_key_expr_doc(&prop.key),
            );
            key_region_end = end;
            parts.push(doc);
        } else {
            self.push_pre_name_comments_doc(&mut parts, cursor, key_start);
            key_region_end = prop.key.span().end;
            // A non-computed field key is unquoted when it is a valid identifier,
            // the same rule as an object property key (`'x' = 1` → `x = 1`). Prettier
            // leaves class field keys quoted — a cataloged divergence (tsv is
            // consistent with its own object/type/interface unquoting).
            parts.push(self.build_property_key_doc(&prop.key));
        }

        // The modifier's marker byte, derived once so the freeze below and the emission
        // that follows it can never disagree about which marker this property carries.
        let marker = match prop.modifier {
            internal::PropertyModifier::None => None,
            internal::PropertyModifier::Optional => Some(b'?'),
            internal::PropertyModifier::Definite => Some(b'!'),
        };

        // An alone-on-line format-ignore directive in the key→marker gap precedes the whole
        // `?: type` / `!: type` tail, so the freeze starts at the marker and swallows it —
        // neither the marker nor the annotation is emitted separately then. A directive
        // AFTER the marker declines here and is routed by the annotation head's own ask.
        let after_modifier = if let Some((frozen, tail_end)) = self
            .build_frozen_marker_annotation_tail(
                key_region_end,
                marker,
                prop.type_annotation.as_ref(),
            ) {
            parts.push(frozen);
            tail_end
        } else {
            // Optional/definite modifier after key, with comment extraction.
            // `push_modifier_marker_doc` also captures comments between key and marker
            // (e.g., `a /* c */? = 1;`); `None` simply has no marker to emit.
            let after_marker = match marker {
                Some(marker) => self.push_modifier_marker_doc(&mut parts, key_region_end, marker),
                None => key_region_end,
            };
            // Type annotation - width-aware wrapping for generics and union types,
            // handling a before-`:` comment between the modifier (or key) and `:`
            // (`c! /* c */ : number`) — line → indented continuation, block → inline.
            if let Some(type_ann) = &prop.type_annotation {
                parts.push(self.build_binding_type_annotation_doc(after_marker, type_ann, true));
            }
            after_marker
        };

        // Value if present - use assignment layout (matches prettier's printAssignment)
        if let Some(value) = &prop.value {
            let before_eq = prop
                .type_annotation
                .as_ref()
                .map_or(after_modifier, |ta| ta.span.end);
            let value_start = value.span().start;
            let eq_pos = self.find_equals_position(before_eq, value_start);
            // Resolved ONCE for both value paths below — see `property_value_facts`.
            let facts = self.property_value_facts(value, eq_pos);

            // A line comment between the LHS and `=` keeps the comment in place and
            // drops `= value` to a continuation line indented one level (preserve —
            // lossless when a second comment also trails the member; prettier relocates
            // it to end-of-line and merges the two — conformance_prettier_ts_comments.md
            // §Comment relocation). Bypasses the assignment layout; value built lazily so the
            // common no-comment path is unaffected.
            let preserve = self.build_initializer_line_continuation(
                before_eq,
                eq_pos,
                ContinuationValue::Expression(value),
                || {
                    let value_doc = self.build_property_value_doc(value, facts);
                    self.prepend_rhs_comments(value_doc, eq_pos + 1, value_start)
                },
            );
            if let Some(cont) = preserve {
                parts.push(cont);
            } else {
                self.build_property_assignment_layout(&mut parts, before_eq, eq_pos, value, facts);
            }
        }

        // Comments between last content and `;`, with the `;` bound to the member: a
        // same-line block trails *after* it (`x = 1 /* c */;` → `x = 1; /* c */`,
        // prettier 3.9), a same-line line trails after it via `line_suffix`, an own-line
        // comment drops to its own line after it (emitting a line comment before the `;`
        // would swallow it). See `push_semicolon_with_gap_comments`.
        let content_end = prop
            .value
            .as_ref()
            .map(|v| v.span().end)
            .or_else(|| prop.type_annotation.as_ref().map(|ta| ta.span.end))
            .unwrap_or(after_modifier);
        self.push_semicolon_with_gap_comments(&mut parts, content_end, prop.span.end, true, None);

        d.concat(&parts)
    }

    /// The `=`→value freeze verdict and the position's clarity-paren answer for a class
    /// field's value, resolved together.
    ///
    /// Both are properties of the VALUE and its position — the `=`→value gap's *content*
    /// cannot change either — so the two paths that print a field's value ask once, at the
    /// caller, and are handed the answer. Resolving them independently is how the
    /// continuation arm (taken when a comment precedes the `=`) came to print an
    /// unparenthesized, **unfrozen** value where the ordinary arm below printed a
    /// parenthesized, frozen one: `p = // c⏎bbb = ccc` against `p = /* c */ (bbb  =  ccc)`,
    /// the directive's whole effect gone at the first spelling.
    fn property_value_facts(
        &self,
        value: &internal::Expression<'_>,
        eq_pos: u32,
    ) -> PropertyValueFacts {
        PropertyValueFacts {
            frozen: self.value_head_frozen_span(eq_pos + 1, value.span()),
            position_parens: self.needs_parens(value, super::ParenContext::DefaultValue),
        }
    }

    /// A class field's value doc: the verbatim slice where its `=`→value gap froze it, else
    /// the ordinary expression doc, inside the clarity parens the position supplies
    /// (`a = (this.a = b);` — built manually like object property values, since the layout
    /// chooser takes the bare expression).
    fn build_property_value_doc(
        &self,
        value: &internal::Expression<'_>,
        facts: PropertyValueFacts,
    ) -> DocId {
        // Every arm reaching here emits its own `= value` shell instead of routing
        // through [`Printer::build_assignment_layout`], which marks the value itself — so
        // they owe the mark here. A class property is one of prettier's
        // `shouldIndentIfInlining` parents (`PropertyDefinition` /
        // `ClassPrivateProperty`, binaryish.js:117-121), and the mark is the only way a
        // binary value learns it: without it the comment arms printed a chain indented
        // while the no-comment arm printed the same property flush.
        self.mark_assignment_value(value);
        let doc = match facts.frozen {
            Some(frozen) => self.build_frozen_expression_doc(value, frozen),
            None => self.build_expression_doc(value),
        };
        if facts.position_parens {
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// Emit a class property's `= value` layout into `parts` (which already holds the
    /// property's LHS). The line-comment-before-`=` fast path is handled by the caller;
    /// this covers before-`=` block comments, a line comment after `=`, and the
    /// no-comment / inline-block assignment layout.
    ///
    /// `facts` is resolved by the caller and shared with that fast path — see
    /// [`Self::property_value_facts`] for why the two arms must be handed one answer.
    fn build_property_assignment_layout(
        &self,
        parts: &mut DocBuf,
        before_eq: u32,
        eq_pos: u32,
        value: &internal::Expression<'_>,
        facts: PropertyValueFacts,
    ) {
        let d = self.d();
        let value_start = value.span().start;

        // Comments before `=` stay before `=` (e.g., `b /* c */ = 1;`)
        if self.has_comments_to_emit_between(before_eq, eq_pos) {
            parts.push(self.build_inline_comments_between_doc(before_eq, eq_pos));
        }

        let PropertyValueFacts {
            frozen: value_frozen,
            position_parens,
        } = facts;
        let build_value = || self.build_property_value_doc(value, facts);

        // Comments after `=`
        if self.has_line_comments_between(eq_pos + 1, value_start) {
            // A same-line comment stays inline with `=` (line comment via
            // `line_suffix`, so its width never force-breaks a preceding type
            // union); own-line comments stay on their own lines (not merged);
            // the value is indented on the next line. `= // comment\n      c`.
            parts.push(d.text(" ="));
            let expr_doc = build_value();
            self.append_keyword_value_line_comments(parts, eq_pos + 1, value_start, expr_doc);
        } else {
            // Use assignment layout for proper line-breaking (handles
            // both no-comment and inline block comment cases).
            // Inline block comments are passed as rhs_comments so
            // choose_layout still applies (e.g., ternary with binaryish
            // test → BreakAfterOperator).
            let rhs_comments = self.build_value_gap_comments_opt(eq_pos + 1, value_start);
            let left_doc = d.concat(&parts[..]);
            let assignment_doc = if position_parens {
                let value_doc = build_value();
                let value_doc = match rhs_comments {
                    Some(comments_doc) => d.concat(&[comments_doc, value_doc]),
                    None => value_doc,
                };
                d.concat(&[left_doc, d.text(" = "), value_doc])
            } else {
                self.build_assignment_layout(
                    left_doc,
                    d.text(" ="),
                    value,
                    AssignmentLeft::Plain,
                    RhsCommentInfo {
                        comments: rhs_comments,
                        has_line_comment: false,
                        pinned: self.comment_run_glued_through(eq_pos + 1, value_start),
                        boundary: None,
                        frozen: value_frozen,
                    },
                )
            };
            *parts = smallvec![assignment_doc];
        }
    }

    /// Build a Doc for a method definition
    fn build_method_definition_doc(&self, method: &internal::MethodDefinition<'_>) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];

        // Decorators (inline or own-line depending on original source)
        let next_token_start = method
            .decorators
            .as_ref()
            .and_then(|decs| decs.last())
            .map_or(method.span.start, |dec| {
                self.find_first_token_after(dec.span.end)
            });
        if let Some(dec_doc) =
            self.build_class_member_decorators_doc(method.decorators, next_token_start)
        {
            parts.push(dec_doc);
        }

        // Modifier keywords, preserving comments between them and before the
        // name (e.g., `static /* c */ async m()`). `cursor` tracks the scan
        // position so each comment is emitted at the user's placement.
        let key_start = method.key.span().start;
        let mut cursor = next_token_start;

        // Accessibility → static → abstract → override, the run shared with properties
        // (a method has no `declare` form, so the run starts at accessibility here)
        self.push_class_member_modifiers_doc(
            &mut parts,
            ClassMemberModifiers {
                accessibility: method.accessibility,
                is_static: method.is_static,
                r#abstract: method.r#abstract,
                r#override: method.r#override,
            },
            &mut cursor,
            key_start,
        );

        // Async modifier
        if method.value.r#async {
            self.push_member_keyword_doc(&mut parts, "async ", &mut cursor, key_start);
        }

        // Generator marker (owns the `*` and comment handling around it)
        if method.value.generator {
            self.push_generator_star_doc(&mut parts, cursor, key_start, method.computed);
        }

        // Get/set for accessors
        match method.kind {
            internal::MethodKind::Get => {
                self.push_member_keyword_doc(&mut parts, "get ", &mut cursor, key_start);
            }
            internal::MethodKind::Set => {
                self.push_member_keyword_doc(&mut parts, "set ", &mut cursor, key_start);
            }
            _ => {}
        }

        // Key
        let key_region_end;
        if method.computed {
            // Comments before the `[` (inside-bracket comments are handled by
            // the bracket builder); generators handle this span after the `*`.
            if !method.value.generator {
                let bracket_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    cursor as usize,
                    key_start as usize,
                    b'[',
                )
                .map_or(key_start, |p| p as u32);
                self.push_pre_name_comments_doc(&mut parts, cursor, bracket_pos);
            }
            // Parenthesize an assignment-expression computed key (`[(x = 0)]() {}`)
            // and an `in` key inside a for-header init, via the shared object/class
            // computed-key helper.
            let (doc, end) = self.build_computed_key_bracket_doc(
                cursor,
                &method.key,
                Some(super::ParenContext::ComputedPropertyKey),
                || self.build_computed_key_expr_doc(&method.key),
            );
            key_region_end = end;
            parts.push(doc);
        } else {
            if !method.value.generator {
                self.push_pre_name_comments_doc(&mut parts, cursor, key_start);
            }
            key_region_end = method.key.span().end;
            // A non-computed method / accessor key is unquoted when it is a valid
            // identifier, the same rule as an object property key (`'foo'() {}` →
            // `foo() {}`). This matches prettier for methods/accessors; a string-keyed
            // `'constructor'` unquotes to the real `constructor` with identical meaning.
            parts.push(self.build_property_key_doc(&method.key));
        }

        // Optional marker: `m?()` (abstract / ambient / interface methods),
        // preserving comments between name and `?` (e.g., `m /* c */?()`)
        let after_key = if method.optional {
            self.push_modifier_marker_doc(&mut parts, key_region_end, b'?')
        } else {
            key_region_end
        };

        // Comments between key/`?` and next token: `[x] /* c */()` or `method/* c */ <T>()`
        self.push_signature_head_comments(
            &mut parts,
            after_key,
            method.value.type_parameters.as_ref(),
            method.value.params_start,
        );

        // Type parameters if present: method<T>()
        if let Some(type_params) = &method.value.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Parameters and return type - shared callable-signature builder (same path
        // as function declarations; MethodDefinition.value is field-identical).
        let (sig_doc, sig_end) = self.build_callable_signature_doc(
            method.value.params,
            method.value.type_parameters.as_ref(),
            method.value.return_type.as_ref(),
            method.value.params_start,
            method.value.body.span.start,
        );
        self.append_signature_head_gap_comments(
            &mut parts,
            self.type_params_paren_gap(method.value.type_parameters.as_ref()),
            None,
            sig_doc,
        );

        // Overload signatures have empty body (start == end)
        let is_overload_signature = method.value.body.span.start == method.value.body.span.end;

        // For abstract methods or overload signatures, use semicolon instead of body
        if method.r#abstract || is_overload_signature {
            // Comments between the return type (or params `)`) and the `;`, with the
            // `;` bound to the member: a same-line trailing block trails *after* it
            // (`a(): number; /* c */`, prettier 3.9 #18837) — unlike interface and
            // type-literal members, which keep a same-line block *before* the `;`
            // (so this class path does not use the shared `append_signature_end_comments`).
            // See `push_semicolon_with_gap_comments`.
            let content_end = method.value.return_type.as_ref().map_or_else(
                || {
                    self.find_closing_paren(method.value.params_start, method.span.end)
                        .unwrap_or(method.span.end)
                },
                |rt| rt.span.end,
            );
            self.push_semicolon_with_gap_comments(
                &mut parts,
                content_end,
                method.span.end,
                true,
                None,
            );
        } else {
            self.append_body_with_sig_comments(&mut parts, sig_end, &method.value.body);
        }

        d.concat(&parts)
    }
}
