// Function declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Build doc for function declaration params with comment handling
    ///
    /// Uses shared implementation and wraps in a group for independent breaking.
    /// `force_break` is used when signature width estimation determines params should break.
    fn build_function_decl_params_doc(
        &self,
        decl: &internal::FunctionDeclaration,
        force_break: bool,
    ) -> Doc {
        let params_start = Some(decl.params_start);

        // Compute trailing comments boundary
        let trailing_comments_end = if let Some(rt) = &decl.return_type {
            Some(rt.span.start)
        } else {
            Some(decl.body.span.start)
        };

        // Use shared implementation with force_break and wrap in group
        let params_doc = self.build_params_doc_with_comments_ext(
            &decl.params,
            params_start,
            trailing_comments_end,
            force_break,
        );
        doc::group(params_doc)
    }

    /// Build a Doc for a function declaration
    pub(super) fn build_function_declaration_doc(
        &self,
        decl: &internal::FunctionDeclaration,
    ) -> Doc {
        let mut parts = Vec::new();
        if decl.r#async {
            parts.push(doc::text("async "));
        }
        parts.push(doc::text("function"));
        if decl.generator {
            parts.push(doc::text("*"));
        }
        if let Some(id) = &decl.id {
            let id_str = self.resolve_symbol(id.name);
            parts.push(doc::text(" "));
            parts.push(doc::text_owned(id_str));
        } else {
            // Prettier adds a space before () for anonymous functions
            parts.push(doc::text(" "));
        }
        // Type parameters (TypeScript generics): function foo<T>()
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }
        // Build params with comment handling
        parts.push(self.build_function_decl_params_doc(decl, false));

        // Return type annotation (e.g., `: number`)
        if let Some(return_type) = &decl.return_type {
            parts.push(self.build_type_annotation_doc(return_type));
        }

        parts.push(doc::text(" "));
        parts.push(self.build_block_statement_doc(&decl.body));

        doc::concat(parts)
    }
}
