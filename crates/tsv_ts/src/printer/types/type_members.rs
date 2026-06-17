// Type member printing for TypeScript
//
// Handles printing of type literal members (TSTypeElement):
// - PropertySignature: `prop: Type`
// - MethodSignature: `method(args): Return`
// - CallSignature: `(args): Return`
// - ConstructSignature: `new (args): Return`
// - IndexSignature: `[key: Type]: Value`

use super::super::comments_in_range;
use super::CommentSpacing;
use super::Printer;
use crate::ast::internal::{self, TSTypeElement};
use crate::printer::analysis::skip_identifier_at;
use tsv_lang::SymbolToU32;
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
    /// not optional (block inline, a line comment forcing a break), and type→end
    /// (`: A /* c */`).
    pub(crate) fn build_property_signature_member_doc(
        &self,
        prop: &internal::TSPropertySignature,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![];
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
            self.build_type_member_key_doc(prop.span.start, &prop.key, prop.computed, true);
        parts.push(key_doc);

        // Push the optional `?` marker (comments around it stay after `?`; prettier
        // moves them before). key_region_end is after `]` for computed keys.
        let after_marker = if prop.optional {
            self.push_modifier_marker_doc(&mut parts, key_region_end, b'?')
        } else {
            key_region_end
        };
        if let Some(type_ann) = &prop.type_annotation {
            let colon_pos = type_ann.span.start;
            // Width-aware wrapping for TypeReference with type arguments.
            let type_doc = self.build_type_annotation_doc_wrapping(type_ann);
            // Comments between the key (or `?`) and `:`. Gate on `has_comments_between`
            // so the common no-comment path stays a single binary search.
            if self.has_comments_between(after_marker, colon_pos) {
                // A line comment keeps the comment after the marker and indents the
                // `: type` continuation one level (`a // c⏎\t\t: T`). A block stays
                // inline before `:`: the optional `?→:` path keeps a space
                // (`a? /* c */ : T`), the non-optional key→`:` path does not
                // (`a /* c */: T`).
                if let Some(doc) =
                    self.build_marker_colon_line_continuation(after_marker, colon_pos, type_doc)
                {
                    parts.push(doc);
                } else {
                    if prop.optional {
                        if let Some(comment_doc) =
                            self.build_marker_to_colon_comments_doc(after_marker, colon_pos)
                        {
                            parts.push(comment_doc);
                        }
                    } else if let Some(comment_doc) = self.build_name_to_type_params_comments_opt(
                        after_marker,
                        colon_pos,
                        CommentSpacing::Leading,
                    ) {
                        parts.push(comment_doc);
                    }
                    parts.push(type_doc);
                }
            } else {
                parts.push(type_doc);
            }

            // Handle comments between type and semicolon (e.g., `a: A /* block */;`)
            for comment in comments_in_range(self.comments, type_ann.span.end, prop.span.end) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
        d.concat(&parts)
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
        method: &internal::TSMethodSignature,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![];
        // Print accessor keyword for get/set signatures, preserving comments
        // between keyword and name.
        match method.kind {
            internal::MethodKind::Get => self.push_accessor_keyword_doc(
                &mut parts,
                "get ",
                method.span.start,
                method.key.span().start,
            ),
            internal::MethodKind::Set => self.push_accessor_keyword_doc(
                &mut parts,
                "set ",
                method.span.start,
                method.key.span().start,
            ),
            _ => {}
        }
        let (key_doc, key_region_end) =
            self.build_type_member_key_doc(method.span.start, &method.key, method.computed, false);
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
        parts.push(self.build_name_to_type_params_comments(
            after_key,
            comments_boundary,
            CommentSpacing::for_type_params(method.type_parameters.is_some()),
        ));

        // Print type parameters if present: `<T>` or `<T, U>`
        if let Some(type_params) = &method.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Comments between type_params `>` and `(` go after type_params
        if let (Some(tp_end), Some(paren_pos)) = (type_params_end, paren_pos) {
            self.append_type_params_to_paren_comments(&mut parts, tp_end, paren_pos);
        }

        parts.push(self.build_signature_params_doc(&method.params, paren_pos));
        if let Some(return_type) = &method.return_type {
            parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
        }
        // Comments between return type (or params) and `;`
        self.append_signature_end_comments(
            &mut parts,
            method.return_type.as_ref(),
            paren_pos,
            method.span.end,
        );
        d.group(d.concat(&parts))
    }

    /// Build a `TSCallSignature` member (`<T>`? `(params)` `: Ret`?) **without**
    /// the trailing `;` — shared by the type-literal and interface type-element
    /// printers (the interface caller appends `;`).
    pub(crate) fn build_call_signature_member_doc(
        &self,
        call: &internal::TSCallSignatureDeclaration,
    ) -> DocId {
        self.build_call_or_construct_signature_doc(
            call.type_parameters.as_ref(),
            &call.params,
            call.return_type.as_ref(),
            call.span.start,
            call.span.end,
            None,
        )
    }

    /// Build a `TSConstructSignature` member (`new` `<T>`? `(params)` `: Ret`?)
    /// **without** the trailing `;` — shared by the type-literal and interface
    /// type-element printers (the interface caller appends `;`).
    pub(crate) fn build_construct_signature_member_doc(
        &self,
        ctor: &internal::TSConstructSignatureDeclaration,
    ) -> DocId {
        self.build_call_or_construct_signature_doc(
            ctor.type_parameters.as_ref(),
            &ctor.params,
            ctor.return_type.as_ref(),
            ctor.span.start,
            ctor.span.end,
            Some(ctor.span.start + "new".len() as u32),
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
        type_parameters: Option<&internal::TSTypeParameterDeclaration>,
        params: &[internal::Expression],
        return_type: Option<&internal::TSTypeAnnotation>,
        span_start: u32,
        span_end: u32,
        new_keyword_end: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![];

        // `new ` prefix + its comment handling (construct signatures only).
        if let Some(new_end) = new_keyword_end {
            parts.push(d.text("new "));
            // Comments between `new` and `<T>`: `new /* c */ <T>(...)`
            if let Some(type_params) = type_parameters
                && let Some(doc) = self.build_name_to_type_params_comments_opt(
                    new_end,
                    type_params.span.start,
                    CommentSpacing::Trailing,
                )
            {
                parts.push(doc);
            }
        }

        // Print type parameters if present: `<T>` or `<T, U>`
        if let Some(type_params) = type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Find `(` (skip comments so a `(` inside one isn't matched).
        let paren_search_start = type_parameters.map_or(span_start, |tp| tp.span.end);
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        // Comments between type_params and `(` go after type_params
        if let (Some(tp), Some(pp)) = (type_parameters.map(|t| t.span.end), paren_pos) {
            self.append_type_params_to_paren_comments(&mut parts, tp, pp);
        }

        // Construct signature without type params: comments between `new` and `(`
        // stay in place.
        if let Some(new_end) = new_keyword_end
            && type_parameters.is_none()
            && let Some(pp) = paren_pos
        {
            for comment in comments_in_range(self.comments, new_end, pp) {
                parts.push(self.build_comment_doc(comment));
                if comment.is_block {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            }
        }

        parts.push(self.build_signature_params_doc(params, paren_pos));
        if let Some(return_type) = return_type {
            parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
        }
        // Comments between return type (or params) and `;`
        self.append_signature_end_comments(&mut parts, return_type, paren_pos, span_end);
        d.group(d.concat(&parts))
    }

    /// Build doc for a type member without its trailing `;` — the type-literal
    /// printer is responsible for the separator and any surrounding comments.
    pub(super) fn build_type_member_doc_inner(&self, member: &TSTypeElement) -> DocId {
        match member {
            TSTypeElement::PropertySignature(prop) => {
                self.build_property_signature_member_doc(prop)
            }
            TSTypeElement::MethodSignature(method) => {
                self.build_method_signature_member_doc(method)
            }
            TSTypeElement::CallSignature(call) => self.build_call_signature_member_doc(call),
            TSTypeElement::ConstructSignature(ctor) => {
                self.build_construct_signature_member_doc(ctor)
            }
            TSTypeElement::IndexSignature(idx) => self.build_index_signature_member_doc(idx),
        }
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
    /// at `[`), `[`→key (`[/* c */ k]` block hugs `[`; a line comment on the `[`
    /// line stays there and breaks the bracket — `[ // c⏎k]`, a `_prettier_divergence`;
    /// an own-line comment stays on its own line inside), key→`:` (`[k /* c */ : T]` block inline;
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
        idx: &internal::TSIndexSignature,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![];

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
        let param_docs: Vec<_> = idx
            .parameters
            .iter()
            .map(|param| {
                let mut param_parts = vec![d.symbol(param.name.to_u32())];
                if let Some(type_ann) = &param.type_annotation {
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
        // `[`→key comment placement: a block comment hugs `[` inline
        // (`[/* c */ k: string]`); a line comment the author wrote *on the `[` line*
        // stays on that line (`[ // c\n\tk: string\n]`) and forces the bracket to
        // break — a divergence from prettier, which relocates it to its own line as
        // the key's leading comment (conformance_prettier.md §Comment relocation,
        // "Object/array/block open-delimiter trailing"). A comment on its own line
        // stays on its own line inside the brackets in both formatters. A comment in
        // the param→`]` gap (`[key: string /* c */]`) trails the contents.
        let (bracket_line_prefix, bracket_pull_pos) = match (bracket_open_pos, first_param_start) {
            (Some(open), Some(key_start)) => self.delimiter_line_comment_prefix(open, key_start),
            _ => (Vec::new(), None),
        };
        // Own-line leading comments stay inside the brackets; a comment pulled onto
        // the `[` line above (same source line as `[`) is emitted by the prefix, so
        // skip it here to avoid emitting it twice.
        // The trailing `.then(!is_empty)` already collapses the no-comment (and
        // all-pulled-onto-the-`[`-line) case to `None`, so no `has_comments_between`
        // guard is needed here (unlike `trailing_comment` below, which has no such net).
        let lead_comment = match (bracket_open_pos, first_param_start) {
            (Some(open), Some(key_start)) => {
                let mut lead_parts = Vec::new();
                for comment in comments_in_range(self.comments, open + 1, key_start) {
                    if let Some(dpos) = bracket_pull_pos
                        && self.comment_on_delimiter_line(dpos, comment)
                    {
                        continue;
                    }
                    lead_parts.push(self.build_comment_doc(comment));
                    if comment.is_block {
                        lead_parts.push(d.text(" "));
                    } else {
                        lead_parts.push(d.hardline());
                    }
                }
                (!lead_parts.is_empty()).then(|| d.concat(&lead_parts))
            }
            _ => None,
        };
        // Comments in the key-type→`]` gap. A block stays inline (`[k: T /* c */]`);
        // a line comment forces the bracket to break and is preserved before `]` — a
        // same-line comment trails the type (`[\n\tk: T // c\n]`), an own-line comment
        // keeps its own line (`[\n\tk: T\n\t// c\n]`). Prettier instead relocates an
        // own-line comment to after `]` (`[k: T] // c`); tsv preserves placement
        // (conformance_prettier.md §Comment relocation), and a line comment swallowing
        // the `]` would otherwise be content loss.
        let (trailing_comment, trailing_has_line) = match bracket_close_pos {
            Some(cp) if self.has_comments_between(search_start, cp) => {
                let mut tparts = Vec::new();
                let mut has_line = false;
                let mut prev = search_start;
                for comment in comments_in_range(self.comments, search_start, cp) {
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
        let mut inner_parts = Vec::new();
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

        // Detect comments between `]` and the value `:` (search only up to the colon,
        // not the type start). Emission is below, after the value type is built.
        let val_colon_pos = idx.type_annotation.span.start;
        let mut has_bracket_colon_comment = false;
        let mut bracket_colon_has_line = false;
        if let Some(close_pos) = bracket_close_pos {
            for comment in comments_in_range(self.comments, close_pos + 1, val_colon_pos) {
                has_bracket_colon_comment = true;
                bracket_colon_has_line |= !comment.is_block;
            }
        }

        // Build the value type annotation. Both branches delegate to the shared
        // `build_type_annotation_doc`, which owns the value-`:`→type comment handling
        // (a line comment breaks + indents so the `//` can't swallow the type), the
        // redundant comment-free paren stripping, and the union (break-after-`:` to
        // leading-`|`) / intersection (hug `:`, continuations wrap) layouts. The only
        // difference is the `]`→value-`:` comment, emitted here: it sits after `]`
        // (prettier relocates it into the brackets).
        let val_annotation = self.build_type_annotation_doc(&idx.type_annotation);
        match bracket_close_pos {
            Some(close_pos) if has_bracket_colon_comment && bracket_colon_has_line => {
                // A line comment in this gap: the first comment trails `]` on its line,
                // then the remaining comments and the value `:` drop to continuation
                // lines indented one level (uniform forced-continuation indent). Each
                // line comment ends its own line via `build_trailing_comments_break_for_line`
                // so a `//` can't swallow the next comment or the `: V` (content loss).
                // Mirrors the `: Type` line-comment layout in `build_type_annotation_doc`.
                parts.push(d.indent(d.concat(&[
                    d.text(" "),
                    self.build_trailing_comments_break_for_line(close_pos + 1, val_colon_pos),
                    val_annotation,
                ])));
            }
            Some(close_pos) if has_bracket_colon_comment => {
                // Block-only comment(s): stay inline before the value `:`, which keeps
                // its own line (`[k: T] /* c */ : V`).
                for comment in comments_in_range(self.comments, close_pos + 1, val_colon_pos) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                parts.push(d.text(" "));
                parts.push(val_annotation);
            }
            _ => parts.push(val_annotation),
        }

        d.concat(&parts)
    }
}
