// Type member printing for TypeScript
//
// Handles printing of type literal members (TSTypeElement):
// - PropertySignature: `prop: Type`
// - MethodSignature: `method(args): Return`
// - CallSignature: `(args): Return`
// - ConstructSignature: `new (args): Return`
// - IndexSignature: `[key: Type]: Value`

use super::super::comments_to_emit_in_range;
use super::CommentSpacing;
use super::Printer;
use crate::ast::internal::{self, TSTypeElement};
use crate::printer::LeadingGlue;
use crate::printer::analysis::skip_identifier_at;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Build a `TSPropertySignature` member (`readonly`? key `?`? `: Type`?)
    /// **without** the trailing `;` — shared verbatim by the type-literal and
    /// interface type-element printers (the interface caller appends `;`; the
    /// type-literal caller leaves the separator to `build_type_literal_doc`).
    ///
    /// Comment handling at each gap: keyword→key (`readonly /* c */ a`),
    /// key→`?` (`a /* c */?`), `?`→`:` (preserved after `?`, a line comment
    /// forcing a break via `build_marker_to_colon_comments_doc`), key→`:` when
    /// not optional (block inline, a line comment forcing a break), and type→`;`
    /// (the pre-`;` gap): a same-line block stays inline (`: A /* c */;`), a
    /// same-line line trails past the `;` (`: A; // c`), and an **own-line**
    /// comment is *deferred* — pushed to `deferred` (own line, blank preserved)
    /// for the type-element joiner to emit **after** its `;`, matching prettier
    /// (the member doc doesn't own the `;`). `deferred` is empty on the common
    /// no-comment path.
    pub(crate) fn build_property_signature_member_doc(
        &self,
        prop: &internal::TSPropertySignature<'_>,
        deferred: &mut DocBuf,
    ) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];
        if prop.readonly {
            // Preserve comments after the keyword (e.g., `readonly /* c */ a`);
            // bounded at `[` for computed keys (inner comments are the
            // bracket builder's)
            let key_start = prop.key.span().start;
            let mut cursor = prop.span.start;
            self.push_member_keyword_doc(&mut parts, "readonly ", &mut cursor, key_start);
            let bound = if prop.computed {
                find_char_skipping_comments(
                    self.source.as_bytes(),
                    cursor as usize,
                    key_start as usize,
                    b'[',
                )
                .map_or(cursor, |pos| pos as u32)
            } else {
                key_start
            };
            self.push_pre_name_comments_doc(&mut parts, cursor, bound);
        }
        let (key_doc, key_region_end) =
            self.build_type_member_key_doc(prop.span.start, &prop.key, prop.computed);
        parts.push(key_doc);

        // Where the trailing (pre-`;`) comment gap begins: after the type annotation
        // if present, else after the key/`?` marker (the no-annotation gap).
        //
        // An alone-on-line format-ignore directive in the key→`?` gap precedes the whole
        // `?: type` tail, so the freeze starts at the marker and swallows it — neither the
        // marker nor the annotation is emitted separately then. A directive AFTER the
        // marker declines here and is routed by the annotation head's own ask.
        let trailing_start = if let Some((frozen, tail_end)) = self
            .build_frozen_marker_annotation_tail(
                key_region_end,
                prop.optional.then_some(b'?'),
                prop.type_annotation.as_ref(),
            ) {
            parts.push(frozen);
            tail_end
        } else {
            // Push the optional `?` marker (comments around it stay after `?`; prettier
            // moves them before). key_region_end is after `]` for computed keys.
            let after_marker = if prop.optional {
                self.push_modifier_marker_doc(&mut parts, key_region_end, b'?')
            } else {
                key_region_end
            };
            match &prop.type_annotation {
                Some(type_ann) => {
                    self.push_property_signature_annotation_doc(
                        &mut parts,
                        after_marker,
                        prop.optional,
                        type_ann,
                    );
                    type_ann.span.end
                }
                None => after_marker,
            }
        };

        // Comments in the content→`;` gap, shared by both arms: with an annotation
        // (`a: A /* c */;`) or in the no-annotation marker→`;` gap (`a /* c */;`,
        // `a? /* c */;`), where they'd otherwise be dropped. Same-line comments stay
        // with the member (a block inline, a line via `line_suffix`); an own-line
        // comment is deferred to `deferred` for the joiner to emit after the `;`
        // (matching prettier). Prettier relocates a no-annotation block before `?` for
        // the optional case — a cataloged divergence (we preserve the author's position).
        deferred.extend(self.split_member_terminator_gap_comments(
            &mut parts,
            trailing_start,
            prop.span.end,
        ));
        d.concat(&parts)
    }

    /// Push a property signature's `: Type` annotation into `parts`, with the comments in
    /// the key- (or `?`-) →`:` gap that precede it. `after_marker` is where that gap opens
    /// (past the `?` when `optional`, else past the key/`]`).
    ///
    /// The frozen route comes first: an alone-on-line format-ignore directive in the gap
    /// freezes the whole `: type` annotation and keeps its own line, asked before the
    /// annotation doc is built at all — the frozen route emits a verbatim slice, so the
    /// built doc would only be discarded.
    fn push_property_signature_annotation_doc(
        &self,
        parts: &mut DocBuf,
        after_marker: u32,
        optional: bool,
        type_ann: &internal::TSTypeAnnotation<'_>,
    ) {
        if let Some(frozen) = self.build_frozen_annotation_head_doc(after_marker, type_ann) {
            parts.push(frozen);
            return;
        }
        let colon_pos = type_ann.span.start;
        // Width-aware wrapping for TypeReference with type arguments.
        let type_doc = self.build_type_annotation_doc_wrapping(type_ann);
        // Comments between the key (or `?`) and `:`. Gate on `has_comments_to_emit_between`
        // so the common no-comment path stays a single binary search.
        if !self.has_comments_to_emit_between(after_marker, colon_pos) {
            parts.push(type_doc);
            return;
        }
        // A line comment keeps the comment after the marker and indents the
        // `: type` continuation one level (`a // c⏎\t\t: T`). A block stays
        // inline before `:`: the optional `?→:` path keeps a space
        // (`a? /* c */ : T`), the non-optional key→`:` path does not
        // (`a /* c */: T`).
        if let Some(doc) =
            self.build_marker_colon_line_continuation(after_marker, colon_pos, type_doc)
        {
            parts.push(doc);
            return;
        }
        let comment_doc = if optional {
            self.build_marker_to_colon_comments_doc(after_marker, colon_pos)
        } else {
            self.build_name_to_type_params_comments_opt(
                after_marker,
                colon_pos,
                CommentSpacing::Leading,
            )
        };
        if let Some(comment_doc) = comment_doc {
            parts.push(comment_doc);
        }
        parts.push(type_doc);
    }

    /// Build a `TSMethodSignature` member (`get`/`set`? key `?`? `<T>`?
    /// `(params)` `: Ret`?) **without** the trailing `;` — shared by the
    /// type-literal and interface type-element printers (the interface caller
    /// appends `;`; the type-literal caller leaves the separator to
    /// `build_type_literal_doc`).
    ///
    /// Comment handling at each gap: accessor keyword→key (`get /* c */ a()`),
    /// key→`?` (`a /* c */?`), `?`/key→`<`/`(` (preserved after `?`; prettier
    /// moves it before `?`, or into the parens for a body-less signature — a
    /// line comment forces a hardline). A comment *inside* `<>` is left to the
    /// type-param doc — the gap search is bounded at `<`, not `>`, so it isn't
    /// emitted twice. Then `>`→`(` and signature end→`;`.
    pub(crate) fn build_method_signature_member_doc(
        &self,
        method: &internal::TSMethodSignature<'_>,
        deferred: &mut DocBuf,
    ) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];
        // Print accessor keyword for get/set signatures, preserving comments
        // between keyword and name.
        match method.kind {
            internal::MethodKind::Get => self.push_accessor_keyword_doc(
                &mut parts,
                "get ",
                method.span.start,
                method.key.span().start,
                method.computed,
            ),
            internal::MethodKind::Set => self.push_accessor_keyword_doc(
                &mut parts,
                "set ",
                method.span.start,
                method.key.span().start,
                method.computed,
            ),
            _ => {}
        }
        let (key_doc, key_region_end) =
            self.build_type_member_key_doc(method.span.start, &method.key, method.computed);
        parts.push(key_doc);

        // Find `(` in source (skip comments so a `(` inside one isn't matched).
        // key_region_end is after `]` for computed keys.
        let type_params_end = method.type_parameters.as_ref().map(|tp| tp.span.end);
        let paren_search_start = type_params_end.unwrap_or(key_region_end);
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        // Optional `?` marker, preserving comments around it: a comment the user
        // wrote *after* `?` stays after it (prettier moves it before `?`, or
        // into the parens for a body-less signature).
        let after_key = if method.optional {
            self.push_modifier_marker_doc(&mut parts, key_region_end, b'?')
        } else {
            key_region_end
        };

        // Comments between key/`?` and the type params `<` (or `(` if none). The
        // boundary is `<`, not `>`: a comment *inside* `<>` belongs to the
        // type-param doc below, and including it here would emit it twice. Line
        // comments get a hardline to prevent absorbing the type params/params as
        // comment text.
        let comments_boundary = method
            .type_parameters
            .as_ref()
            .map(|tp| tp.span.start)
            .or(paren_pos)
            .unwrap_or(key_region_end);
        self.push_name_to_type_params_comments(
            &mut parts,
            after_key,
            comments_boundary,
            CommentSpacing::for_type_params(method.type_parameters.is_some()),
        );

        // Print type parameters if present: `<T>` or `<T, U>`
        if let Some(type_params) = &method.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        let sig_doc = self.build_signature_params_return_group(
            method.params,
            method.type_parameters.as_ref(),
            method.return_type.as_ref(),
            paren_pos,
        );
        // Comments between type_params `>` and `(` go after type_params
        self.append_signature_head_gap_comments(
            &mut parts,
            type_params_end.zip(paren_pos),
            d.empty(),
            sig_doc,
        );
        // Comments between return type (or params) and `;`
        self.append_signature_end_comments(
            &mut parts,
            method.return_type.as_ref(),
            paren_pos,
            method.span.end,
            deferred,
        );
        d.group(d.concat(&parts))
    }

    /// Build a `TSCallSignature` member (`<T>`? `(params)` `: Ret`?) **without**
    /// the trailing `;` — shared by the type-literal and interface type-element
    /// printers (the interface caller appends `;`).
    pub(crate) fn build_call_signature_member_doc(
        &self,
        call: &internal::TSCallSignatureDeclaration<'_>,
        deferred: &mut DocBuf,
    ) -> DocId {
        self.build_call_or_construct_signature_doc(
            call.type_parameters.as_ref(),
            call.params,
            call.return_type.as_ref(),
            call.span,
            None,
            deferred,
        )
    }

    /// Build a `TSConstructSignature` member (`new` `<T>`? `(params)` `: Ret`?)
    /// **without** the trailing `;` — shared by the type-literal and interface
    /// type-element printers (the interface caller appends `;`).
    pub(crate) fn build_construct_signature_member_doc(
        &self,
        ctor: &internal::TSConstructSignatureDeclaration<'_>,
        deferred: &mut DocBuf,
    ) -> DocId {
        self.build_call_or_construct_signature_doc(
            ctor.type_parameters.as_ref(),
            ctor.params,
            ctor.return_type.as_ref(),
            ctor.span,
            Some(ctor.span.start + "new".len() as u32),
            deferred,
        )
    }

    /// Shared core for call and construct signature members. The two declarations
    /// are field-identical (`type_parameters` / `params` / `return_type` / `span`)
    /// and differ only by the `new` prefix on construct signatures.
    ///
    /// `new_keyword_end`: `Some(pos)` (the offset just past `new`) for a construct
    /// signature, `None` for a call signature. When set, the doc gets a leading
    /// `new ` plus that signature's comment handling — comments after `new` go
    /// before `<T>` (`new /* c */ <T>`), or, when there are no type params, before
    /// `(` (`new /* c */ (a)`, preserved in place; prettier relocates them into the
    /// parens). The `new ` text carries the leading space, so blocks get only a
    /// trailing space and line comments a hardline.
    fn build_call_or_construct_signature_doc(
        &self,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        params: &[internal::Expression<'_>],
        return_type: Option<&internal::TSTypeAnnotation<'_>>,
        span: internal::Span,
        new_keyword_end: Option<u32>,
        deferred: &mut DocBuf,
    ) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];

        // `new` prefix + its comment handling (construct signatures only).
        //
        // With type parameters the `new`→`<T>` gap is its own seam and the `>`→`(` gap
        // below is the head gap. WITHOUT them the `new`→`(` gap IS the head gap, so it
        // routes through the same shared emitter — its own separator deferred to that
        // call, which is why the keyword is pushed bare here. It used to emit its run
        // inline with a flush continuation, disagreeing with the constructor TYPE's
        // identical `new`→`(` gap (`type C = new // c⏎\t(p: A) => A`) on the one axis
        // §Uniform Forced-Continuation Indent makes uniform.
        let new_paren_gap = match (new_keyword_end, type_parameters) {
            (Some(new_end), None) => {
                parts.push(d.text("new"));
                Some(new_end)
            }
            (Some(new_end), Some(type_params)) => {
                parts.push(d.text("new "));
                // Comments between `new` and `<T>`: `new /* c */ <T>(...)`
                if let Some(doc) = self.build_name_to_type_params_comments_opt(
                    new_end,
                    type_params.span.start,
                    CommentSpacing::Trailing,
                ) {
                    parts.push(doc);
                }
                None
            }
            (None, _) => None,
        };

        // Print type parameters if present: `<T>` or `<T, U>`
        if let Some(type_params) = type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Find `(` (skip comments so a `(` inside one isn't matched).
        let paren_search_start = type_parameters.map_or(span.start, |tp| tp.span.end);
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        let sig_doc = self.build_signature_params_return_group(
            params,
            type_parameters,
            return_type,
            paren_pos,
        );
        // The signature HEAD gap — `>`→`(` with type parameters, `new`→`(` without. The two
        // differ only in what separates the head from the tail when nothing breaks: `<T>(` is
        // glued, `new (` is not. A CALL signature has neither head, and takes no separator —
        // its `(` opens the member.
        let (head, flat_separator) = match (type_parameters.map(|t| t.span.end), new_paren_gap) {
            (Some(type_params_end), _) => (Some(type_params_end), d.empty()),
            (None, Some(new_end)) => (Some(new_end), d.text(" ")),
            (None, None) => (None, d.empty()),
        };
        self.append_signature_head_gap_comments(
            &mut parts,
            head.zip(paren_pos),
            flat_separator,
            sig_doc,
        );
        // Comments between return type (or params) and `;`
        self.append_signature_end_comments(&mut parts, return_type, paren_pos, span.end, deferred);
        d.group(d.concat(&parts))
    }

    /// Build doc for a type member without its trailing `;` — the type-literal
    /// printer is responsible for the separator and any surrounding comments.
    /// `deferred` collects own-line comments in a member's content→`;` gap that must
    /// render **after** the joiner's `;` (matching prettier, since the member doc
    /// doesn't own the `;`); empty on the common no-comment path.
    pub(in crate::printer) fn build_type_member_doc_inner(
        &self,
        member: &TSTypeElement<'_>,
        deferred: &mut DocBuf,
    ) -> DocId {
        match member {
            TSTypeElement::PropertySignature(prop) => {
                self.build_property_signature_member_doc(prop, deferred)
            }
            TSTypeElement::MethodSignature(method) => {
                self.build_method_signature_member_doc(method, deferred)
            }
            TSTypeElement::CallSignature(call) => {
                self.build_call_signature_member_doc(call, deferred)
            }
            TSTypeElement::ConstructSignature(ctor) => {
                self.build_construct_signature_member_doc(ctor, deferred)
            }
            TSTypeElement::IndexSignature(idx) => {
                self.build_index_signature_member_doc(idx, deferred)
            }
        }
    }

    /// Wrap a member doc with its own trailing `;` followed by the own-line comments
    /// that deferred past the `;` (`deferred`). This is the **interface / class**
    /// member idiom, where each member carries its own separator (unlike the
    /// type-literal joiner, which owns the `;` and emits `deferred` in its member
    /// loop). `deferred` is empty on the common no-comment path.
    pub(in crate::printer) fn build_member_with_semicolon_doc(
        &self,
        member: DocId,
        deferred: DocBuf,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![member, d.text(";")];
        parts.extend(deferred);
        d.concat(&parts)
    }

    /// Build a `TSIndexSignature` member (`static`? `readonly`? `[key: KeyType]`
    /// `: Value`) **without** the trailing `;` — shared by the type-literal,
    /// interface, and class index-signature printers (the interface and class
    /// callers append `;`; the type-literal caller leaves the separator to
    /// `build_type_literal_doc`), matching how the property/method/call/construct
    /// members already delegate. `static` is class-only (`is_static` is always
    /// false for type-element members).
    ///
    /// Comment handling at each gap: keyword→`[` (`readonly /* c */ [k]`, bounded
    /// at `[`), `[`→key (the shared leading-comment run, keyed on what follows the
    /// `*/`: a glued block leads the key inline — `[/* c */ k]` — a newline after the
    /// `*/` takes the soft `line` so the key pulls up only while the bracket fits, and
    /// an own-line comment keeps its own line inside with any author blank preserved;
    /// a line comment on the `[` line stays there and breaks the bracket —
    /// `[ // c⏎k]`, a `_prettier_divergence`), key→`:` (`[k /* c */ : T]` block inline;
    /// `[k // c⏎: T]` line forces a hardline that breaks the bracket, so the `//`
    /// can't swallow the `: T`), type→`]` (`[k: T /* c */]` block inline; a line
    /// comment breaks the bracket and is preserved before `]` — same-line trailing
    /// the type, own-line on its own line — a `_prettier_divergence` since prettier
    /// relocates an own-line comment to after `]`),
    /// and `]`→`:` (`[k: T] /* c */ : V` block inline; a line comment stays after
    /// `]` and drops the value `:` to the next line, indented one level — a
    /// `_prettier_divergence`, prettier relocates it into the brackets trailing the key type.
    /// Multiple comments here each keep their own line — the first trails `]`, the rest
    /// indent with the value `:` — so a `//` can't swallow the next, `[k: T] // a⏎// b⏎: V`).
    /// The value type — colon→type comments (block inline, line comments breaking)
    /// and the union/intersection break layout, including redundant-paren stripping
    /// — is delegated to the shared `build_type_annotation_doc`.
    pub(in crate::printer) fn build_index_signature_member_doc(
        &self,
        idx: &internal::TSIndexSignature<'_>,
        deferred: &mut DocBuf,
    ) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];

        // Locate the opening `[`, skipping comments so a `[` inside one (e.g.
        // `readonly /* [ */ [k]`) isn't matched. Bounded at the first parameter so
        // a `[` in the key type can't be mistaken for it.
        let first_param_start = idx.parameters.first().map(|p| p.span.start);
        let bracket_bound = first_param_start.unwrap_or(idx.span.end);
        let bracket_open_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            idx.span.start as usize,
            bracket_bound as usize,
            b'[',
        )
        .map(|p| p as u32);

        if idx.is_static || idx.readonly {
            // Modifier keywords (`static`/`readonly`, the former class-only),
            // preserving comments before each and before the `[`
            // (e.g., `static /* c */ readonly /* d */ [k: string]: T`).
            let mut cursor = idx.span.start;
            if idx.is_static {
                self.push_member_keyword_doc(&mut parts, "static ", &mut cursor, bracket_bound);
            }
            if idx.readonly {
                self.push_member_keyword_doc(&mut parts, "readonly ", &mut cursor, bracket_bound);
            }
            self.push_pre_name_comments_doc(&mut parts, cursor, bracket_open_pos.unwrap_or(cursor));
        }

        // Build the key parameter docs. The `: keyType` is delegated to the
        // shared annotation printer so a union/intersection key breaks with the
        // leading-`|` / hanging-`&` layout.
        let param_docs: DocBuf = idx
            .parameters
            .iter()
            .map(|param| {
                // An alone-on-line directive in the `[`→key gap freezes the key
                // parameter it precedes (Rule A) — the `]: V` value side, which the
                // directive does not precede, keeps formatting. The gap is asked
                // directly rather than through `list_item_frozen`: an index signature
                // has exactly one parameter (a second is a parse error), so there is no
                // inter-item gap and the `[` anchor is the only one.
                if bracket_open_pos
                    .is_some_and(|open| self.member_gap_frozen(open + 1, param.span.start))
                {
                    return self.build_frozen_span_doc(param.span);
                }
                let mut param_parts: DocBuf = smallvec![self.identifier_name_doc(param)];
                if let Some(type_ann) = param.type_annotation() {
                    // The `: keyType` annotation, handling a before-`:` comment between
                    // the key name and `:` — line → indented continuation (the hardline
                    // also breaks the bracket group), block → inline (`[key /* c */ : T]`).
                    // The annotation itself (colon→type comments, union/intersection
                    // break layout) is delegated to the shared annotation printer.
                    let name_end = skip_identifier_at(
                        self.source.as_bytes(),
                        param.span.start as usize,
                        type_ann.span.start as usize,
                    ) as u32;
                    param_parts
                        .push(self.build_binding_type_annotation_doc(name_end, type_ann, false));
                }
                d.concat(&param_parts)
            })
            .collect();

        // The closing `]`, located outside comments so a `]` glyph inside a
        // comment before it (`[key: string /* ] */]`) isn't mistaken for it.
        let search_start = idx.parameters.last().map_or(idx.span.start, |p| p.span.end);
        let bracket_close_pos = self.find_char_outside_comments(search_start, idx.span.end, b']');

        // Build `[key: type]` as a group that can break when key type is long
        // Flat: [key: type]
        // Break: [\n\tkey: type\n]
        //
        // `[`→key comment placement: a line comment the author wrote *on the `[` line*
        // stays on that line (`[ // c\n\tk: string\n]`) and forces the bracket to
        // break — a divergence from prettier, which relocates it to its own line as
        // the key's leading comment (conformance_prettier_ts_comments.md §Comment relocation,
        // "Object/array/block open-delimiter trailing"). Every other comment in the
        // gap is the leading run below, which both formatters agree on. A comment in
        // the param→`]` gap (`[key: string /* c */]`) trails the contents.
        let (bracket_line_prefix, bracket_pull_pos) = match (bracket_open_pos, first_param_start) {
            (Some(open), Some(key_start)) => self.delimiter_line_comment_prefix(open, key_start),
            _ => (DocBuf::new(), None),
        };
        // Own-line leading comments stay inside the brackets; a comment pulled onto
        // the `[` line above (same source line as `[`) is emitted by the prefix, so
        // skip it here to avoid emitting it twice.
        // The trailing `.then(!is_empty)` already collapses the no-comment (and
        // all-pulled-onto-the-`[`-line) case to `None`, so no `has_comments_to_emit_between`
        // guard is needed here (unlike `trailing_comment` below, which has no such net).
        let lead_comment = match (bracket_open_pos, first_param_start) {
            (Some(open), Some(key_start)) => {
                // The shared leading-comment emitter, so the `[`→key gap gets the one
                // rule: a block glued to what follows leads the key inline
                // (`[/* c */ k: string]`, and a run the author glued stays glued), a
                // block with a newline after its `*/` takes the soft `line` (the key
                // pulls up when the bracket group fits and drops below when it breaks),
                // and an own-line comment or any line comment takes the
                // blank-preserving hardline — which also breaks the bracket group.
                let mut lead_parts = DocBuf::new();
                self.push_leading_comment_run(
                    &mut lead_parts,
                    comments_to_emit_in_range(self.comments, open + 1, key_start).filter(|c| {
                        !bracket_pull_pos
                            .is_some_and(|dpos| self.comment_on_delimiter_line(dpos, c))
                    }),
                    key_start,
                    LeadingGlue::Adjacent,
                    d.empty(),
                );
                (!lead_parts.is_empty()).then(|| d.concat(&lead_parts))
            }
            _ => None,
        };
        // Comments in the key-type→`]` gap. A block stays inline (`[k: T /* c */]`);
        // a line comment forces the bracket to break and is preserved before `]` — a
        // same-line comment trails the type (`[\n\tk: T // c\n]`), an own-line comment
        // keeps its own line (`[\n\tk: T\n\t// c\n]`). Prettier instead relocates an
        // own-line comment to after `]` (`[k: T] // c`); tsv preserves placement
        // (conformance_prettier_ts_comments.md §Comment relocation), and a line comment swallowing
        // the `]` would otherwise be content loss.
        let (trailing_comment, trailing_has_line) = match bracket_close_pos {
            Some(cp) if self.has_comments_to_emit_between(search_start, cp) => {
                let mut tparts = DocBuf::new();
                let mut has_line = false;
                let mut prev = search_start;
                for comment in comments_to_emit_in_range(self.comments, search_start, cp) {
                    if self.is_same_line(prev, comment.span.start) {
                        tparts.push(d.text(" "));
                    } else {
                        tparts.push(d.hardline());
                    }
                    tparts.push(self.build_comment_doc(comment));
                    has_line |= !comment.is_block;
                    prev = comment.span.end;
                }
                (Some(d.concat(&tparts)), has_line)
            }
            _ => (None, false),
        };
        let mut inner_parts = DocBuf::new();
        inner_parts.extend(lead_comment);
        inner_parts.push(d.join(param_docs, ", "));
        inner_parts.extend(trailing_comment);
        let bracket_inner = d.concat(&inner_parts);
        let bracket_body = d.concat(&[
            d.text("["),
            d.concat(&bracket_line_prefix),
            d.indent_softline(bracket_inner),
            d.softline(),
            d.text("]"),
        ]);
        // A same-line `[` comment pulled onto the `[` line, or any line comment in the
        // key-type→`]` gap, forces the bracket to break so the `//` can't swallow the
        // key or `]` (the group would otherwise stay flat); other breaks are width- or
        // inner-comment-driven via `group`.
        let bracket_group = if bracket_pull_pos.is_some() || trailing_has_line {
            d.group_break(bracket_body)
        } else {
            d.group(bracket_body)
        };
        parts.push(bracket_group);

        // Build the value type annotation when present, then record where the
        // content→`;` gap begins for the shared terminator handling below. A typeless
        // index signature (`[key: string]`) has no value `:`/type, so its gap starts
        // just past `]`.
        let gap_start = if let Some(type_annotation) = &idx.type_annotation {
            // `]`→value-`:` is the before-`:` gap in its bracketed spelling, so it takes the
            // shared binding seam rather than a fourth hand-rolled copy of it: the same
            // frozen-head check (an alone-on-line format-ignore directive keeps its own line
            // and freezes the whole `: type` — trailing it on the `]` line is inert and
            // loses the freeze on pass 2), the same continuation indent for a hanging
            // comment, the same inline run for glued blocks (`[k: T] /* c */ : V`), and the
            // same `build_type_annotation_doc` body for everything past the `:`. Prettier
            // relocates this one into the brackets — see
            // [§Uniform Forced-Continuation Indent](../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent),
            // which already lists this site beside the key/binding/parameter spellings.
            //
            // The seam needs the gap's left edge, so a `]` we could not locate falls back to
            // the bare annotation — the arm the hand-rolled `match` also ended on.
            parts.push(match bracket_close_pos {
                Some(close_pos) => {
                    self.build_binding_type_annotation_doc(close_pos + 1, type_annotation, false)
                }
                None => self.build_type_annotation_doc(type_annotation),
            });
            type_annotation.span.end
        } else {
            bracket_close_pos.map_or(idx.span.end, |cp| cp + 1)
        };

        // Comments in the content→`;` gap (`[k: string]: T /* c */;`, or a typeless
        // `[k: string] /* c */;`). Without this they were dropped (content loss) —
        // nothing else covers this gap: the member doc ends at the content, and the
        // joiner's `content_end` starts at the `;`. Same-line comments stay before the
        // `;` (a block inline, a line via `line_suffix`); an own-line comment defers to
        // `deferred` for the joiner to emit after the `;`, matching prettier.
        deferred.extend(self.split_member_terminator_gap_comments(
            &mut parts,
            gap_start,
            idx.span.end,
        ));

        d.concat(&parts)
    }
}
