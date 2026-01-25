// Type member printing for TypeScript
//
// Handles printing of type literal members (TSTypeElement):
// - PropertySignature: `prop: Type`
// - MethodSignature: `method(args): Return`
// - CallSignature: `(args): Return`
// - ConstructSignature: `new (args): Return`
// - IndexSignature: `[key: Type]: Value`

use super::super::comments_in_range;
use super::Printer;
use super::helpers::intersection_has_huggable_last_type;
use crate::ast::internal::{self, TSType, TSTypeElement};
use tsv_lang::SymbolToU32;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Build doc for type member with optional trailing semicolon
    pub(super) fn build_type_member_doc_inner(
        &self,
        member: &TSTypeElement,
        with_semicolon: bool,
    ) -> Doc {
        match member {
            TSTypeElement::PropertySignature(prop) => {
                let mut parts = vec![];
                if prop.readonly {
                    parts.push(doc::text("readonly "));
                }
                if prop.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&prop.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&prop.key));
                }

                // Handle comments between key and colon (e.g., `b /* comment */: B`)
                let key_end = prop.key.span().end;
                if let Some(type_ann) = &prop.type_annotation {
                    let type_ann_start = type_ann.span.start;
                    for comment in comments_in_range(self.comments, key_end, type_ann_start) {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }

                if prop.optional {
                    parts.push(doc::text("?"));
                }
                if let Some(type_ann) = &prop.type_annotation {
                    // Use width-aware wrapping for TypeReference with type arguments
                    parts.push(self.build_type_annotation_doc_wrapping(type_ann));

                    // Handle comments between type and semicolon (e.g., `a: A /* block */;`)
                    let type_end = type_ann.span.end;
                    let prop_end = prop.span.end;
                    for comment in comments_in_range(self.comments, type_end, prop_end) {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }

                    // For simple types with trailing comments, the doc already includes semicolon
                    let has_trailing_comment =
                        self.type_annotation_has_trailing_comment_doc(type_ann);
                    if with_semicolon && !has_trailing_comment {
                        parts.push(doc::text(";"));
                    }
                } else if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
            TSTypeElement::MethodSignature(method) => {
                let mut parts = vec![];
                // Print accessor keyword for get/set signatures
                match method.kind {
                    internal::MethodKind::Get => parts.push(doc::text("get ")),
                    internal::MethodKind::Set => parts.push(doc::text("set ")),
                    _ => {}
                }
                if method.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&method.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&method.key));
                }

                // Handle comments around method signature parts
                // Comments between key and type_params/`(` go before `?`
                // Comments between type_params and `(` go after type_params
                let key_end = method.key.span().end;
                let type_params_end = method.type_parameters.as_ref().map(|tp| tp.span.end);

                // Find the position of `(` in source (search after type_params if present)
                let paren_search_start = type_params_end.unwrap_or(key_end);
                let paren_pos = self.source[paren_search_start as usize..]
                    .find('(')
                    .map(|p| paren_search_start + p as u32);

                // Comments between key and type_params (or `(` if no type_params) go before `?`
                let comments_before_boundary = type_params_end.or(paren_pos).unwrap_or(key_end);
                for comment in comments_in_range(self.comments, key_end, comments_before_boundary) {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }

                if method.optional {
                    parts.push(doc::text("?"));
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &method.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Comments between type_params and `(` go after type_params
                if let (Some(tp_end), Some(paren_pos)) = (type_params_end, paren_pos) {
                    for comment in comments_in_range(self.comments, tp_end, paren_pos) {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }

                parts.push(self.build_signature_params_doc(&method.params, paren_pos));
                if let Some(return_type) = &method.return_type {
                    parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::group(doc::concat(parts))
            }
            TSTypeElement::CallSignature(call) => {
                let mut parts = vec![];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &call.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Find paren position for comment handling
                let paren_search_start = call
                    .type_parameters
                    .as_ref()
                    .map_or(call.span.start, |tp| tp.span.end);
                let paren_pos = self.source[paren_search_start as usize..]
                    .find('(')
                    .map(|p| paren_search_start + p as u32);

                parts.push(self.build_signature_params_doc(&call.params, paren_pos));
                if let Some(return_type) = &call.return_type {
                    parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::group(doc::concat(parts))
            }
            TSTypeElement::ConstructSignature(ctor) => {
                let mut parts = vec![doc::text("new ")];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &ctor.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Find paren position for comment handling
                let paren_search_start = ctor
                    .type_parameters
                    .as_ref()
                    .map_or(ctor.span.start, |tp| tp.span.end);
                let paren_pos = self.source[paren_search_start as usize..]
                    .find('(')
                    .map(|p| paren_search_start + p as u32);

                parts.push(self.build_signature_params_doc(&ctor.params, paren_pos));
                if let Some(return_type) = &ctor.return_type {
                    parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::group(doc::concat(parts))
            }
            TSTypeElement::IndexSignature(idx) => {
                self.build_type_element_index_signature_doc(idx, with_semicolon)
            }
        }
    }

    /// Build doc for index signature in type elements: `[key: Type]: Value`
    fn build_type_element_index_signature_doc(
        &self,
        idx: &internal::TSIndexSignature,
        with_semicolon: bool,
    ) -> Doc {
        let mut parts = vec![];
        if idx.readonly {
            parts.push(doc::text("readonly "));
        }

        // Build the key parameter docs
        // For key type annotations with unions/intersections, use special handling
        // so they break properly with leading |/trailing & style.
        let param_docs: Vec<_> = idx
            .parameters
            .iter()
            .map(|param| {
                let mut param_parts = vec![doc::symbol(param.name.to_u32())];
                if let Some(type_ann) = &param.type_annotation {
                    match type_ann.type_annotation.as_ref() {
                        TSType::Union(u) => {
                            // Union key type: break after `:` with leading `|`
                            let type_doc = self.build_union_type_doc(u, false);
                            param_parts.push(doc::text(":"));
                            param_parts.push(doc::group(doc::indent(doc::concat(vec![
                                doc::line(),
                                type_doc,
                            ]))));
                        }
                        TSType::Intersection(i) => {
                            // Intersection key type: break after `:` with trailing `&`
                            let type_doc = self.build_intersection_type_doc(i, false);
                            param_parts.push(doc::text(":"));
                            param_parts.push(doc::group(doc::indent(doc::concat(vec![
                                doc::line(),
                                type_doc,
                            ]))));
                        }
                        _ => {
                            // Regular key type: use standard annotation
                            param_parts.push(self.build_type_annotation_doc(type_ann));
                        }
                    }
                }
                doc::concat(param_parts)
            })
            .collect();

        // Build `[key: type]` as a group that can break when key type is long
        // Flat: [key: type]
        // Break: [\n\tkey: type\n]
        let bracket_contents = doc::join(param_docs, ", ");
        let bracket_group = doc::group(doc::concat(vec![
            doc::text("["),
            doc::indent_softline(bracket_contents),
            doc::softline(),
            doc::text("]"),
        ]));
        parts.push(bracket_group);

        // Handle comments between `]` and type annotation
        // Search for `]` from the last parameter's end position
        let search_start = idx.parameters.last().map_or(idx.span.start, |p| p.span.end);
        let bracket_close_pos = self.source[search_start as usize..]
            .find(']')
            .map(|p| search_start + p as u32);
        let type_start = idx.type_annotation.type_annotation.span().start;
        let mut has_comment = false;
        if let Some(close_pos) = bracket_close_pos {
            // Search up to the type's span start, not the annotation's
            for comment in comments_in_range(self.comments, close_pos + 1, type_start) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
                has_comment = true;
            }
        }

        // Build value type annotation with proper breaking for long unions/intersections
        // When we have a comment before `:`, we handle `:` ourselves to avoid
        // build_type_annotation_doc outputting the comment again.
        if has_comment {
            // We already output the comment, now just add ` : Type`
            parts.push(doc::text(" : "));
            parts.push(self.build_type_doc(&idx.type_annotation.type_annotation));
        } else {
            // No comment before `:`, use normal type annotation handling
            match idx.type_annotation.type_annotation.as_ref() {
                TSType::Union(u) => {
                    let type_doc = self.build_union_type_doc(u, false);
                    parts.push(doc::text(":"));
                    parts.push(doc::group(doc::indent(doc::concat(vec![
                        doc::line(), // space when flat, newline when broken
                        type_doc,
                    ]))));
                }
                TSType::Intersection(i) => {
                    let type_doc = self.build_intersection_type_doc(i, false);
                    if intersection_has_huggable_last_type(i) {
                        // No indent/line - keep `: Type & {` hugged
                        parts.push(doc::text(": "));
                        parts.push(type_doc);
                    } else {
                        parts.push(doc::text(":"));
                        parts.push(doc::group(doc::indent(doc::concat(vec![
                            doc::line(),
                            type_doc,
                        ]))));
                    }
                }
                _ => {
                    parts.push(self.build_type_annotation_doc(&idx.type_annotation));
                }
            }
        }

        if with_semicolon {
            parts.push(doc::text(";"));
        }
        doc::concat(parts)
    }
}
