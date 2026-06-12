// Function declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use crate::printer::CommentSpacing;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::source_scan::find_char_skipping_comments;

use super::super::types::function_types::{
    return_type_triggers_grouping, type_params_allow_grouping,
};

/// Prettier's `shouldGroupFunctionParameters`: wrap params in their own group
/// when there's 1 param and the return type is an object type or will break.
///
/// This lets params stay flat even when the outer signature group breaks
/// due to a multiline return type.
fn should_group_function_parameters(
    decl: &internal::FunctionDeclaration,
    return_type_doc: Option<DocId>,
    d: &DocArena,
) -> bool {
    if decl.params.len() != 1 {
        return false;
    }
    let Some(rt_doc) = return_type_doc else {
        return false;
    };
    if !type_params_allow_grouping(decl.type_parameters.as_ref()) {
        return false;
    }
    decl.return_type
        .as_ref()
        .is_some_and(|rt| return_type_triggers_grouping(rt, rt_doc, d))
}

impl<'a> Printer<'a> {
    /// Build doc for function signature (params + return type) with comment handling.
    ///
    /// When `should_group_function_parameters` is true, params are wrapped in their
    /// own inner group so they can stay flat even when the outer group breaks due to
    /// the return type's hardlines.
    fn build_function_signature_doc(&self, decl: &internal::FunctionDeclaration) -> DocId {
        let d = self.d();
        let params_start = Some(decl.params_start);

        // Params trailing comments are bounded at the close paren; a comment between
        // `)` and the return type is emitted via build_paren_to_return_type_comments.
        let trailing_comments_end =
            Some(self.params_trailing_comments_end(decl.params_start, decl.body.span.start));

        let params_doc = self.build_params_doc_with_comments_ext(
            &decl.params,
            params_start,
            trailing_comments_end,
            false,
        );

        let return_type_doc = decl
            .return_type
            .as_ref()
            .map(|rt| self.build_type_annotation_doc_for_return_type(rt));

        let params_doc = if should_group_function_parameters(decl, return_type_doc, d) {
            d.group(params_doc)
        } else {
            params_doc
        };

        let mut sig_parts = vec![params_doc];
        if let Some(rt_doc) = return_type_doc {
            // Preserve a comment between `)` and the return type `:` in place.
            if let Some(rt) = &decl.return_type {
                sig_parts.push(
                    self.build_paren_to_return_type_comments(
                        Some(decl.params_start),
                        rt.span.start,
                    ),
                );
            }
            sig_parts.push(rt_doc);
        }

        d.group(d.concat(&sig_parts))
    }

    /// Build a Doc for a function declaration
    pub(super) fn build_function_declaration_doc(
        &self,
        decl: &internal::FunctionDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let search_end = decl
            .id
            .as_ref()
            .map_or(decl.params_start, |id| id.span.start);
        let mut cursor = decl.span.start;

        if decl.r#async {
            parts.push(d.text("async"));
            cursor = decl.span.start + 5; // "async" length
        }

        // Find "function" in source after cursor, skipping comments
        let function_pos = self.find_keyword_in_source(cursor, search_end, "function");
        if let Some(fp) = function_pos {
            if let Some(c) = self.build_inline_comments_between_doc_opt(cursor, fp) {
                parts.push(c);
            }
            if cursor > decl.span.start {
                parts.push(d.text(" "));
            }
            parts.push(d.text("function"));
            cursor = fp + 8; // "function" length
        } else {
            if cursor > decl.span.start {
                parts.push(d.text(" "));
            }
            parts.push(d.text("function"));
        }

        if decl.generator {
            parts.push(d.text("*"));
            cursor += 1;
        }
        if let Some(id) = &decl.id {
            // Comments between function/function* and the name
            parts.push(self.build_keyword_to_name_comments(cursor, id.span.start));
            parts.push(d.symbol(id.name.to_u32()));

            // Comments between name and type params/parens: `function fn1/* c */ <T>()` or `fn1 /* c */()`
            // Line comments get a hardline to prevent absorbing type params as comment text
            let comment_end = decl
                .type_parameters
                .as_ref()
                .map_or(decl.params_start, |tp| tp.span.start);
            parts.push(self.build_name_to_type_params_comments(
                id.span.end,
                comment_end,
                CommentSpacing::for_type_params(decl.type_parameters.is_some()),
            ));
        } else {
            // Anonymous function (export default): extract comments between keyword and params
            // `export default function /* c */ () {}`
            // Line comments get hardline to prevent absorbing parens: `function // c\n()`
            let next_start = decl
                .type_parameters
                .as_ref()
                .map_or(decl.params_start, |tp| tp.span.start);
            parts.push(self.build_keyword_to_name_comments(cursor, next_start));
        }
        // Type parameters (TypeScript generics): function foo<T>()
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));

            // Comments between type_params `>` and `(` go after type_params
            if let Some(pp) = find_char_skipping_comments(
                self.source.as_bytes(),
                type_params.span.end as usize,
                self.source.len(),
                b'(',
            ) {
                self.append_type_params_to_paren_comments(
                    &mut parts,
                    type_params.span.end,
                    pp as u32,
                );
            }
        }

        // Signature (params + return type) in a single group
        parts.push(self.build_function_signature_doc(decl));

        // Handle comments between signature and body: function a() /* comment */ {}
        let sig_end = self.signature_end(
            decl.return_type.as_ref(),
            decl.params_start,
            decl.body.span.start,
        );
        self.append_body_with_sig_comments(&mut parts, sig_end, &decl.body);

        d.concat(&parts)
    }
}
