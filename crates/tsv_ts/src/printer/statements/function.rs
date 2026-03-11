// Function declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::{DocArena, DocId};

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

        // Compute trailing comments boundary
        let trailing_comments_end = if let Some(rt) = &decl.return_type {
            Some(rt.span.start)
        } else {
            Some(decl.body.span.start)
        };

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
        if decl.r#async {
            parts.push(d.text("async "));
        }
        parts.push(d.text("function"));
        if decl.generator {
            parts.push(d.text("*"));
        }
        if let Some(id) = &decl.id {
            parts.push(d.text(" "));
            // Comments between keywords and the name: `async /* a */ function* /* b */ F()`
            // Search from span start to find all comments before the name
            // (prettier normalizes them to after `function*`)
            parts.push(
                self.build_inline_comments_between_doc_trailing_space(
                    decl.span.start,
                    id.span.start,
                ),
            );
            parts.push(d.symbol(id.name.to_u32()));
        } else {
            // Prettier adds a space before () for anonymous functions
            parts.push(d.text(" "));
        }
        // Type parameters (TypeScript generics): function foo<T>()
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Signature (params + return type) in a single group
        parts.push(self.build_function_signature_doc(decl));

        parts.push(d.text(" "));
        parts.push(self.build_block_statement_doc(&decl.body));

        d.concat(&parts)
    }
}
