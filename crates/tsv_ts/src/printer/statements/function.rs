// Function declaration printing for TypeScript

use super::super::Printer;
use crate::ast::internal;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Print a function declaration: `function foo(x) { return x + 1; }`
    /// or anonymous: `function () {}` (Prettier adds space before parenthesis for anonymous functions)
    /// or async: `async function foo() {}`
    /// or generator: `function* foo() {}`
    /// or async generator: `async function* foo() {}`
    ///
    /// Uses hybrid approach: doc-based for signature wrapping, imperative for body (preserves comments).
    pub(super) fn print_function_declaration(&mut self, decl: &internal::FunctionDeclaration) {
        // Print prefix (async, function, *, name) imperatively
        if decl.r#async {
            self.write("async ");
        }
        self.write("function");
        if decl.generator {
            self.write("*");
        }
        if let Some(id) = &decl.id {
            self.write(" ");
            self.print_identifier(id);
        } else {
            // Prettier adds a space before () for anonymous functions
            self.write(" ");
        }

        // Build and print signature (type params + params + return type) using doc-based wrapping
        let sig_doc = self.build_function_signature_doc(decl);
        self.write_doc(&sig_doc);

        // Print body imperatively (preserves comments)
        self.write(" ");
        self.print_block_statement(&decl.body);
    }

    /// Build a Doc for just the function signature (type params + params + return type).
    /// Body is printed separately via imperative printer to preserve comments.
    ///
    /// NOTE: While the doc printer has look-ahead for fits(), function signatures
    /// have independent groups for type params and params. When type params stay
    /// inline but push the total line over print_width, params don't automatically
    /// know to break. We use a heuristic to estimate when this happens.
    fn build_function_signature_doc(&self, decl: &internal::FunctionDeclaration) -> Doc {
        // Check if type params would stay inline but total signature is too long
        let force_params_break = if let Some(tp) = &decl.type_parameters {
            // Type params break if: multiple params OR contains multiline content
            let has_multiple_params = tp.params.len() > 1;
            let span_str = &self.source[tp.span.start as usize..tp.span.end as usize];
            let is_multiline = span_str.contains('\n');
            let type_params_will_break = has_multiple_params || is_multiline;

            if type_params_will_break {
                // Type params break → params get fresh line budget → don't force break
                false
            } else {
                // Estimate total signature width
                let type_params_width = (tp.span.end - tp.span.start) as usize;
                let params_width: usize = decl
                    .params
                    .iter()
                    .map(|p| (p.span().end - p.span().start) as usize + 2)
                    .sum();
                let return_type_width = decl
                    .return_type
                    .as_ref()
                    .map_or(0, |rt| (rt.span.end - rt.span.start) as usize);
                // +4 accounts for parens and spaces: "() " around params
                let estimated_total = 20 + type_params_width + params_width + return_type_width + 4;
                estimated_total > self.config.print_width.saturating_sub(5)
            }
        } else {
            false
        };

        let mut parts = Vec::new();

        // Type parameters handling - always in their own group for independent breaking
        if let Some(tp) = &decl.type_parameters {
            parts.push(self.build_type_params_doc_for_function_grouped(tp));
        }

        // Function parameters
        if force_params_break && !decl.params.is_empty() {
            parts.push(self.build_function_params_doc_force_break(&decl.params));
        } else {
            parts.push(self.build_function_params_doc_grouped(&decl.params));
        }

        // Return type annotation
        if let Some(return_type) = &decl.return_type {
            parts.push(self.build_type_annotation_doc(return_type));
        }

        doc::concat(parts)
    }

    /// Build doc for type params wrapped in their own group
    fn build_type_params_doc_for_function_grouped(
        &self,
        decl: &internal::TSTypeParameterDeclaration,
    ) -> Doc {
        if decl.params.is_empty() {
            return doc::text("<>");
        }

        let param_docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();
        let inner_parts = doc::join_trailing(param_docs, doc::comma_line());

        // Wrap in own group so type params can break independently
        doc::group(doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(inner_parts),
            doc::softline(),
            doc::text(">"),
        ]))
    }

    /// Build doc for function params in their own group (independent breaking)
    fn build_function_params_doc_grouped(&self, params: &[internal::Expression]) -> Doc {
        if params.is_empty() {
            return doc::text("()");
        }

        let param_docs: Vec<_> = params
            .iter()
            .map(|param| self.build_expression_doc(param))
            .collect();
        let inner_parts = vec![doc::join_doc(param_docs, doc::comma_line())];

        // Check if last param is a rest parameter - no trailing comma after rest
        let has_rest_param = params
            .last()
            .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));

        let mut result = vec![
            doc::text("("),
            doc::indent_softline(doc::concat(inner_parts)),
        ];

        // Trailing comma when broken, unless there's a rest param
        if !has_rest_param {
            result.push(doc::trailing_comma());
        }

        result.push(doc::softline());
        result.push(doc::text(")"));

        // Wrap in own group for independent breaking
        doc::group(doc::concat(result))
    }

    /// Build doc for function params that always break (forced by signature width heuristic)
    fn build_function_params_doc_force_break(&self, params: &[internal::Expression]) -> Doc {
        if params.is_empty() {
            return doc::text("()");
        }

        let param_docs: Vec<_> = params
            .iter()
            .map(|param| self.build_expression_doc(param))
            .collect();
        let inner_parts = vec![doc::join_doc(param_docs, doc::comma_hardline())];

        // Check if last param is a rest parameter - no trailing comma after rest
        let has_rest_param = params
            .last()
            .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));

        let mut result = vec![
            doc::text("("),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(inner_parts)])),
        ];

        // Always add trailing comma since we're breaking
        if !has_rest_param {
            result.push(doc::text(","));
        }

        result.push(doc::hardline());
        result.push(doc::text(")"));

        doc::concat(result)
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
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }
        parts.push(doc::text("("));

        // Build params (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        let param_docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_expression_doc(param))
            .collect();
        parts.push(doc::join(param_docs, ", "));

        parts.push(doc::text(")"));

        // Return type annotation (e.g., `: number`)
        if let Some(return_type) = &decl.return_type {
            parts.push(self.build_type_annotation_doc(return_type));
        }

        parts.push(doc::text(" "));
        parts.push(self.build_block_statement_doc(&decl.body));

        doc::concat(parts)
    }
}
