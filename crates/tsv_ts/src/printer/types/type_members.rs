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
use super::helpers::intersection_has_huggable_last_type;
use crate::ast::internal::{self, TSType, TSTypeElement};
use crate::printer::analysis::skip_identifier_at;
use crate::printer::layout::hang_after_operator;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Build doc for a type member without its trailing `;` — the type-literal
    /// printer is responsible for the separator and any surrounding comments.
    pub(super) fn build_type_member_doc_inner(&self, member: &TSTypeElement) -> DocId {
        let d = self.d();
        match member {
            TSTypeElement::PropertySignature(prop) => {
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

                // Handle comments between key and colon (e.g., `b /* comment */: B`)
                // key_region_end is after `]` for computed, avoiding re-finding bracket comments
                if let Some(type_ann) = &prop.type_annotation {
                    let type_ann_start = type_ann.span.start;
                    for comment in comments_in_range(self.comments, key_region_end, type_ann_start)
                    {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }

                if prop.optional {
                    parts.push(d.text("?"));
                }
                if let Some(type_ann) = &prop.type_annotation {
                    // Use width-aware wrapping for TypeReference with type arguments
                    parts.push(self.build_type_annotation_doc_wrapping(type_ann));

                    // Handle comments between type and semicolon (e.g., `a: A /* block */;`)
                    let type_end = type_ann.span.end;
                    let prop_end = prop.span.end;
                    for comment in comments_in_range(self.comments, type_end, prop_end) {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }
                d.concat(&parts)
            }
            TSTypeElement::MethodSignature(method) => {
                let mut parts = vec![];
                // Print accessor keyword for get/set signatures, preserving
                // comments between keyword and name
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
                let (key_doc, key_region_end) = self.build_type_member_key_doc(
                    method.span.start,
                    &method.key,
                    method.computed,
                    false,
                );
                parts.push(key_doc);

                // Handle comments around method signature parts
                // Comments between key and type_params/`(` go before `?`
                // Comments between type_params and `(` go after type_params
                // key_region_end is after `]` for computed, avoiding re-finding bracket comments
                let type_params_end = method.type_parameters.as_ref().map(|tp| tp.span.end);

                // Find the position of `(` in source (skip comments to avoid matching `(` inside them)
                let paren_search_start = type_params_end.unwrap_or(key_region_end);
                let paren_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    paren_search_start as usize,
                    self.source.len(),
                    b'(',
                )
                .map(|p| p as u32);

                // Comments between key and type_params (or `(` if no type_params) go before `?`
                // Line comments get a hardline to prevent absorbing type params as comment text
                let comments_before_boundary =
                    type_params_end.or(paren_pos).unwrap_or(key_region_end);
                parts.push(self.build_name_to_type_params_comments(
                    key_region_end,
                    comments_before_boundary,
                    CommentSpacing::for_type_params(method.type_parameters.is_some()),
                ));

                if method.optional {
                    parts.push(d.text("?"));
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &method.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Comments between type_params and `(` go after type_params
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
            TSTypeElement::CallSignature(call) => {
                let mut parts = vec![];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &call.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Find paren position for comment handling (skip comments to avoid matching `(` inside them)
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
                if let (Some(tp), Some(pp)) =
                    (call.type_parameters.as_ref().map(|t| t.span.end), paren_pos)
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
            TSTypeElement::ConstructSignature(ctor) => {
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

                // Find paren position for comment handling (skip comments to avoid matching `(` inside them)
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
                if let (Some(tp), Some(pp)) =
                    (ctor.type_parameters.as_ref().map(|t| t.span.end), paren_pos)
                {
                    self.append_type_params_to_paren_comments(&mut parts, tp, pp);
                }

                // Without type params, comments between `new` and `(` stay in
                // place: `new /* c */ (a: number)` (prettier relocates them
                // into the parens). The "new " text already carries the
                // leading space, so blocks get only a trailing space and line
                // comments a hardline.
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
                    let comments_doc = self.build_comments_between(
                        val_colon_end,
                        val_type_start,
                        CommentSpacing::Trailing,
                    );
                    let has_line_comments_between_members = i.types.windows(2).any(|p| {
                        self.has_line_comments_between(p[0].span().end, p[1].span().start)
                    });
                    if has_line_comments_between_members {
                        // Keep the first type inline after `:` (prettier does too); the
                        // continuation is indented. `hang_after_operator` would instead
                        // break after `:` because the line comment's forced hardline
                        // turns its leading `line` into a break.
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(self.intersection_hanging_with_indent(i));
                    } else if intersection_has_huggable_last_type(i) {
                        // No indent/line - keep `: Type & {` hugged
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(self.build_intersection_type_doc(i, false));
                    } else {
                        let type_doc = self.build_intersection_type_doc(i, false);
                        parts.push(d.text(":"));
                        parts.push(hang_after_operator(d, d.concat(&[comments_doc, type_doc])));
                    }
                }
                _ => {
                    parts.push(self.build_type_annotation_doc(&idx.type_annotation));
                }
            }
        }

        d.concat(&parts)
    }
}
