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
use super::helpers::{intersection_has_expanding_first_type, intersection_has_huggable_last_type};
use crate::ast::internal::{self, TSType, TSTypeElement};
use crate::printer::analysis::{find_char_skipping_comments, skip_identifier_at};
use crate::printer::layout::hang_after_operator;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build doc for a type member without its trailing `;` — the type-literal
    /// printer is responsible for the separator and any surrounding comments.
    pub(super) fn build_type_member_doc_inner(&self, member: &TSTypeElement) -> DocId {
        let d = self.d();
        match member {
            TSTypeElement::PropertySignature(prop) => {
                let mut parts = vec![];
                if prop.readonly {
                    parts.push(d.text("readonly "));
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
                // Print accessor keyword for get/set signatures
                match method.kind {
                    internal::MethodKind::Get => parts.push(d.text("get ")),
                    internal::MethodKind::Set => parts.push(d.text("set ")),
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
                let content_end = method.return_type.as_ref().map_or_else(
                    || {
                        paren_pos
                            .and_then(|p| self.find_closing_paren(p, method.span.end))
                            .unwrap_or(method.span.end)
                    },
                    |rt| rt.span.end,
                );
                for comment in comments_in_range(self.comments, content_end, method.span.end) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
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
                let content_end = call.return_type.as_ref().map_or_else(
                    || {
                        paren_pos
                            .and_then(|p| self.find_closing_paren(p, call.span.end))
                            .unwrap_or(call.span.end)
                    },
                    |rt| rt.span.end,
                );
                for comment in comments_in_range(self.comments, content_end, call.span.end) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                d.group(d.concat(&parts))
            }
            TSTypeElement::ConstructSignature(ctor) => {
                let mut parts = vec![d.text("new ")];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &ctor.type_parameters {
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

                parts.push(self.build_signature_params_doc(&ctor.params, paren_pos));
                if let Some(return_type) = &ctor.return_type {
                    parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
                }
                // Comments between return type (or params) and `;`
                let content_end = ctor.return_type.as_ref().map_or_else(
                    || {
                        paren_pos
                            .and_then(|p| self.find_closing_paren(p, ctor.span.end))
                            .unwrap_or(ctor.span.end)
                    },
                    |rt| rt.span.end,
                );
                for comment in comments_in_range(self.comments, content_end, ctor.span.end) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
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
            parts.push(d.text("readonly "));
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
                    // Extract comments between param name and colon: `[key /* c */ : string]`
                    // Prettier adds space before `:` when comments present
                    let colon_pos = type_ann.span.start;
                    let name_end = skip_identifier_at(
                        self.source.as_bytes(),
                        param.span.start as usize,
                        colon_pos as usize,
                    ) as u32;
                    let has_pre_colon_comment = if let Some(comment_doc) =
                        self.build_inline_comments_between_doc_opt(name_end, colon_pos)
                    {
                        param_parts.push(comment_doc);
                        true
                    } else {
                        false
                    };
                    let key_colon_end = colon_pos + 1;
                    let key_type_start = type_ann.type_annotation.span().start;
                    // Union/Intersection key types: break after `:` with leading `|` or trailing `&`
                    let breaking_type_doc = match type_ann.type_annotation.as_ref() {
                        TSType::Union(u) => Some(self.build_union_type_doc(u, false)),
                        TSType::Intersection(i) => Some(self.build_intersection_type_doc(i, false)),
                        _ => None,
                    };
                    if let Some(type_doc) = breaking_type_doc {
                        let comments_doc = self.build_comments_between(
                            key_colon_end,
                            key_type_start,
                            CommentSpacing::Trailing,
                        );
                        param_parts.push(d.text(if has_pre_colon_comment { " :" } else { ":" }));
                        param_parts
                            .push(hang_after_operator(d, d.concat(&[comments_doc, type_doc])));
                    } else {
                        // Regular key type: use standard annotation
                        // build_type_annotation_doc emits `: type`, need space before `:` with comments
                        if has_pre_colon_comment {
                            let type_start = type_ann.type_annotation.span().start;
                            let colon_end = colon_pos + 1;
                            param_parts.push(d.text(" :"));
                            // Handle comments between `:` and type (delegate to existing logic)
                            let between_doc =
                                self.build_inline_comments_between_doc(colon_end, type_start);
                            param_parts.push(d.text(" "));
                            param_parts.push(between_doc);
                            param_parts.push(self.build_type_doc(&type_ann.type_annotation));
                        } else {
                            param_parts.push(self.build_type_annotation_doc(type_ann));
                        }
                    }
                }
                d.concat(&param_parts)
            })
            .collect();

        // Build `[key: type]` as a group that can break when key type is long
        // Flat: [key: type]
        // Break: [\n\tkey: type\n]
        let bracket_contents = d.join(param_docs, ", ");
        let bracket_group = d.group(d.concat(&[
            d.text("["),
            d.indent_softline(bracket_contents),
            d.softline(),
            d.text("]"),
        ]));
        parts.push(bracket_group);

        // Handle comments between `]` and `:` of value type annotation
        // Only search up to the colon position, not the type start
        let search_start = idx.parameters.last().map_or(idx.span.start, |p| p.span.end);
        let bracket_close_pos = self.source[search_start as usize..]
            .find(']')
            .map(|p| search_start + p as u32);
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
            // No bracket-colon comment: use normal type annotation handling
            match idx.type_annotation.type_annotation.as_ref() {
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
                    let type_doc = self.build_intersection_type_doc(i, false);
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
                        // turns its leading `line` into a break. Mirrors the line-comment
                        // branch of `build_intersection_type_annotation_doc`.
                        let wrapped = if intersection_has_huggable_last_type(i)
                            || intersection_has_expanding_first_type(i)
                        {
                            type_doc
                        } else {
                            d.group(d.indent(type_doc))
                        };
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(wrapped);
                    } else if intersection_has_huggable_last_type(i) {
                        // No indent/line - keep `: Type & {` hugged
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(type_doc);
                    } else {
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
