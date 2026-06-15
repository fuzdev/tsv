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
use crate::ast::internal::{self, TSType, TSTypeElement};
use crate::printer::analysis::skip_identifier_at;
use crate::printer::layout::hang_after_operator;
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

        // Comments around the optional `?` marker, split so that a comment the
        // user wrote *after* `?` stays after it (prettier moves it before `?`).
        // key_region_end is after `]` for computed, avoiding re-finding bracket
        // comments.
        if prop.optional {
            let after_q = self.push_modifier_marker_doc(&mut parts, key_region_end, b'?');
            if let Some(type_ann) = &prop.type_annotation
                && let Some(comment_doc) =
                    self.build_marker_to_colon_comments_doc(after_q, type_ann.span.start)
            {
                parts.push(comment_doc);
            }
        } else if let Some(type_ann) = &prop.type_annotation {
            // Comments between key and `:` (e.g., `[x] /* c */: number`). A block
            // comment stays inline; a line comment forces a hardline so it can't
            // swallow the `: T` annotation as comment text (`a // c⏎: T`) — a
            // content-loss / non-idempotency fix.
            if let Some(comment_doc) = self.build_name_to_type_params_comments_opt(
                key_region_end,
                type_ann.span.start,
                CommentSpacing::Leading,
            ) {
                parts.push(comment_doc);
            }
        }
        if let Some(type_ann) = &prop.type_annotation {
            // Use width-aware wrapping for TypeReference with type arguments
            parts.push(self.build_type_annotation_doc_wrapping(type_ann));

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
        let d = self.d();
        let mut parts = vec![];
        // Print type parameters if present: `<T>` or `<T, U>`
        if let Some(type_params) = &call.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Find `(` (skip comments so a `(` inside one isn't matched).
        let paren_search_start = call
            .type_parameters
            .as_ref()
            .map_or(call.span.start, |tp| tp.span.end);
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        // Comments between type_params and `(` go after type_params
        if let (Some(tp), Some(pp)) = (call.type_parameters.as_ref().map(|t| t.span.end), paren_pos)
        {
            self.append_type_params_to_paren_comments(&mut parts, tp, pp);
        }

        parts.push(self.build_signature_params_doc(&call.params, paren_pos));
        if let Some(return_type) = &call.return_type {
            parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
        }
        // Comments between return type (or params) and `;`
        self.append_signature_end_comments(
            &mut parts,
            call.return_type.as_ref(),
            paren_pos,
            call.span.end,
        );
        d.group(d.concat(&parts))
    }

    /// Build a `TSConstructSignature` member (`new` `<T>`? `(params)` `: Ret`?)
    /// **without** the trailing `;` — shared by the type-literal and interface
    /// type-element printers (the interface caller appends `;`).
    ///
    /// Comments after `new`: before `<T>` (`new /* c */ <T>`), or — when there
    /// are no type params — before `(` (`new /* c */ (a)`, preserved in place;
    /// prettier relocates them into the parens). The `new ` text carries the
    /// leading space, so blocks get only a trailing space and line comments a
    /// hardline.
    pub(crate) fn build_construct_signature_member_doc(
        &self,
        ctor: &internal::TSConstructSignatureDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("new ")];
        // Print type parameters if present: `<T>` or `<T, U>`
        if let Some(type_params) = &ctor.type_parameters {
            // Comments between `new` and `<T>`: `new /* c */ <T>(...)`
            let new_end = ctor.span.start + 3;
            if let Some(doc) = self.build_name_to_type_params_comments_opt(
                new_end,
                type_params.span.start,
                CommentSpacing::Trailing,
            ) {
                parts.push(doc);
            }
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Find `(` (skip comments so a `(` inside one isn't matched).
        let paren_search_start = ctor
            .type_parameters
            .as_ref()
            .map_or(ctor.span.start, |tp| tp.span.end);
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        // Comments between type_params and `(` go after type_params
        if let (Some(tp), Some(pp)) = (ctor.type_parameters.as_ref().map(|t| t.span.end), paren_pos)
        {
            self.append_type_params_to_paren_comments(&mut parts, tp, pp);
        }

        // Without type params, comments between `new` and `(` stay in place.
        if ctor.type_parameters.is_none()
            && let Some(pp) = paren_pos
        {
            for comment in comments_in_range(self.comments, ctor.span.start + 3, pp) {
                parts.push(self.build_comment_doc(comment));
                if comment.is_block {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            }
        }

        parts.push(self.build_signature_params_doc(&ctor.params, paren_pos));
        if let Some(return_type) = &ctor.return_type {
            parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
        }
        // Comments between return type (or params) and `;`
        self.append_signature_end_comments(
            &mut parts,
            ctor.return_type.as_ref(),
            paren_pos,
            ctor.span.end,
        );
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
            TSTypeElement::IndexSignature(idx) => self.build_type_element_index_signature_doc(idx),
        }
    }

    /// Build doc for index signature in type elements: `[key: Type]: Value`
    fn build_type_element_index_signature_doc(&self, idx: &internal::TSIndexSignature) -> DocId {
        let d = self.d();
        let mut parts = vec![];
        if idx.readonly {
            // Preserve comments before the `[` (e.g., `readonly /* c */ [k: string]: T`)
            let bracket_bound = idx
                .parameters
                .first()
                .map_or(idx.span.end, |p| p.span.start);
            let mut cursor = idx.span.start;
            self.push_member_keyword_doc(&mut parts, "readonly ", &mut cursor, bracket_bound);
            let bracket_pos = find_char_skipping_comments(
                self.source.as_bytes(),
                cursor as usize,
                bracket_bound as usize,
                b'[',
            )
            .map_or(cursor, |p| p as u32);
            self.push_pre_name_comments_doc(&mut parts, cursor, bracket_pos);
        }

        // Build the key parameter docs
        // For key type annotations with unions/intersections, use special handling
        // so they break properly with leading |/trailing & style.
        let param_docs: Vec<_> = idx
            .parameters
            .iter()
            .map(|param| {
                let mut param_parts = vec![d.symbol(param.name.to_u32())];
                if let Some(type_ann) = &param.type_annotation {
                    // Comments between the key name and the colon: `[key /* c */ : string]`.
                    // Prettier adds a space before `:` when such a comment is present.
                    let colon_pos = type_ann.span.start;
                    let name_end = skip_identifier_at(
                        self.source.as_bytes(),
                        param.span.start as usize,
                        colon_pos as usize,
                    ) as u32;
                    if let Some(comment_doc) =
                        self.build_inline_comments_between_doc_opt(name_end, colon_pos)
                    {
                        param_parts.push(comment_doc);
                        param_parts.push(d.text(" "));
                    }
                    // Delegate the `: keyType` — colon→type comments (line comments break,
                    // never merge) and the union/intersection break layout — to the shared
                    // annotation printer.
                    param_parts.push(self.build_type_annotation_doc(type_ann));
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
        // A comment in the param→`]` gap (`[key: string /* c */]`) trails the
        // contents inside the brackets, preserved in place.
        let bracket_contents = d.join(param_docs, ", ");
        let bracket_inner = match bracket_close_pos
            .and_then(|cp| self.build_inline_comments_between_doc_opt(search_start, cp))
        {
            Some(comment) => d.concat(&[bracket_contents, comment]),
            None => bracket_contents,
        };
        let bracket_group = d.group(d.concat(&[
            d.text("["),
            d.indent_softline(bracket_inner),
            d.softline(),
            d.text("]"),
        ]));
        parts.push(bracket_group);

        // Handle comments between `]` and `:` of value type annotation
        // Only search up to the colon position, not the type start
        let val_colon_pos = idx.type_annotation.span.start;
        let val_colon_end = val_colon_pos + 1;
        let val_type_start = idx.type_annotation.type_annotation.span().start;
        let mut has_bracket_colon_comment = false;
        if let Some(close_pos) = bracket_close_pos {
            for comment in comments_in_range(self.comments, close_pos + 1, val_colon_pos) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                has_bracket_colon_comment = true;
            }
        }

        // Build value type annotation with proper breaking for long unions/intersections
        if has_bracket_colon_comment {
            // Bracket-colon comment present: emit ` : ` then handle colon-to-type comments
            parts.push(d.text(" : "));
            parts.push(self.build_comments_between(
                val_colon_end,
                val_type_start,
                CommentSpacing::Trailing,
            ));
            parts.push(self.build_type_doc(&idx.type_annotation.type_annotation));
        } else {
            // No bracket-colon comment: use normal type annotation handling.
            // Strip redundant comment-free parens so `($A | $B)` / `($A & $B)`
            // value types get the same hanging layout as the bare form (prettier
            // strips them too); other parenthesized types keep the `_` fall-through.
            match self.unwrap_redundant_parens(idx.type_annotation.type_annotation.as_ref()) {
                TSType::Union(u) => {
                    let type_doc = self.build_union_type_doc(u, false);
                    let comments_doc = self.build_comments_between(
                        val_colon_end,
                        val_type_start,
                        CommentSpacing::Trailing,
                    );
                    parts.push(d.text(":"));
                    parts.push(hang_after_operator(d, d.concat(&[comments_doc, type_doc])));
                }
                TSType::Intersection(i) => {
                    // Hug `: A & B & …` on the colon line, continuation members wrapping
                    // one level in (`: A &\n\tB &\n\tC`) — prettier keeps intersections on
                    // the colon line, unlike unions (which break after `:` to leading-`|`).
                    // `intersection_hanging_with_indent` stays inline when it fits, handles
                    // line comments between members, and skips its own indent for a huggable
                    // last / expanding first member (avoiding double-indent).
                    let comments_doc = self.build_comments_between(
                        val_colon_end,
                        val_type_start,
                        CommentSpacing::Trailing,
                    );
                    parts.push(d.text(": "));
                    parts.push(comments_doc);
                    parts.push(self.intersection_hanging_with_indent(i));
                }
                _ => {
                    parts.push(self.build_type_annotation_doc(&idx.type_annotation));
                }
            }
        }

        d.concat(&parts)
    }
}
