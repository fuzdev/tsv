// Function declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Build doc for function signature (params + return type) with comment handling.
    ///
    /// Returns a single group containing both params and return type.
    /// This ensures params break BEFORE return type when signature exceeds width.
    fn build_function_signature_doc(&self, decl: &internal::FunctionDeclaration) -> Doc {
        let params_start = Some(decl.params_start);

        // Compute trailing comments boundary
        let trailing_comments_end = if let Some(rt) = &decl.return_type {
            Some(rt.span.start)
        } else {
            Some(decl.body.span.start)
        };

        // Build params doc without wrapping in a group
        let params_doc = self.build_params_doc_with_comments_ext(
            &decl.params,
            params_start,
            trailing_comments_end,
            false,
        );

        let mut sig_parts = vec![params_doc];

        // Return type annotation
        if let Some(return_type) = &decl.return_type {
            sig_parts.push(self.build_type_annotation_doc_for_return_type(return_type));
        }

        // Single outer group for entire signature (params + return type).
        // When this group breaks, params' softlines become newlines while return type stays flat.
        doc::group(doc::concat(sig_parts))
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
            parts.push(doc::text(" "));
            parts.push(doc::symbol(id.name.to_u32()));
        } else {
            // Prettier adds a space before () for anonymous functions
            parts.push(doc::text(" "));
        }
        // Type parameters (TypeScript generics): function foo<T>()
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Signature (params + return type) in a single group
        parts.push(self.build_function_signature_doc(decl));

        parts.push(doc::text(" "));
        parts.push(self.build_block_statement_doc(&decl.body));

        doc::concat(parts)
    }
}
