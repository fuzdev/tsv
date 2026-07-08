// Function declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use crate::printer::CommentSpacing;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

use super::super::types::function_types::group_params_if_should;

impl<'a> Printer<'a> {
    /// Build doc for a callable signature (params + return type) with comment handling.
    ///
    /// Shared by function declarations and class methods — their `FunctionDeclaration`
    /// / `FunctionExpression` payloads carry identical signature fields, so the caller
    /// passes them decomposed. When `should_group_function_parameters` is true, params
    /// are wrapped in their own inner group so they can stay flat even when the outer
    /// group breaks due to the return type's hardlines.
    ///
    /// Delegates the params + return-type core to `build_signature_params_return` and
    /// wraps it (with no type-parameter prefix — the caller builds those separately)
    /// in the signature group. Returns the doc plus the signature end — where comments
    /// before the body begin: the return type's end when present, otherwise just past
    /// the `)` (falling back to `body_start` if the paren can't be located).
    pub(in crate::printer) fn build_callable_signature_doc(
        &self,
        params: &[internal::Expression<'_>],
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        return_type: Option<&internal::TSTypeAnnotation<'_>>,
        params_start: u32,
        body_start: u32,
    ) -> (DocId, u32) {
        let d = self.d();
        let (params_doc, return_type_doc, sig_end) =
            self.build_signature_params_return(params, type_parameters, return_type, params_start, body_start);

        let mut sig_parts: DocBuf = smallvec![params_doc];
        if let Some(rt_doc) = return_type_doc {
            sig_parts.push(rt_doc);
        }

        (d.group(d.concat(&sig_parts)), sig_end)
    }

    /// Build the params + return-type core shared by `build_callable_signature_doc`
    /// (function declarations, class methods) and
    /// `build_function_expression_signature_doc` (function expressions, object methods)
    /// — the two builders differ only in the type-parameter prefix the caller prepends.
    ///
    /// One depth-tracked close-`)` scan feeds every derived boundary: the params doc
    /// (trailing comments bounded at `)` — a comment past it belongs to the `)`→return
    /// gap or the signature→body gap), the combined `)`→`:` return-type doc (the comment
    /// prefix folded into `: T` so the single-param hug sees a will-break comment there),
    /// the hug itself (`group_params_if_should`), and the signature end (the return
    /// type's end, else just past `)`). Returns `(params_doc, return_type_doc, sig_end)`.
    pub(in crate::printer) fn build_signature_params_return(
        &self,
        params: &[internal::Expression<'_>],
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        return_type: Option<&internal::TSTypeAnnotation<'_>>,
        params_start: u32,
        body_start: u32,
    ) -> (DocId, Option<DocId>, u32) {
        let close_paren_after = self.find_closing_paren(params_start, body_start);

        let trailing_comments_end =
            Some(close_paren_after.map_or(body_start, |after_paren| after_paren - 1));

        let params_doc =
            self.build_params_doc_with_comments(params, Some(params_start), trailing_comments_end);

        let return_type_doc =
            return_type.map(|rt| self.build_function_return_type_doc(close_paren_after, rt));

        let params_doc = group_params_if_should(
            params_doc,
            params,
            type_parameters,
            return_type,
            return_type_doc,
            self.d(),
        );

        let sig_end = match return_type {
            Some(rt) => rt.span.end,
            None => close_paren_after.unwrap_or(body_start),
        };

        (params_doc, return_type_doc, sig_end)
    }

    /// Build a Doc for a function declaration
    pub(super) fn build_function_declaration_doc(
        &self,
        decl: &internal::FunctionDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let search_end = decl
            .id
            .as_ref()
            .map_or(decl.params_start, |id| id.span.start);
        let mut cursor = decl.span.start;

        if decl.r#async {
            parts.push(d.text("async"));
            cursor = decl.span.start + "async".len() as u32;
        }

        // Find "function" in source after cursor, skipping comments
        let function_pos = self.find_keyword_in_range(cursor, search_end, "function");
        if let Some(fp) = function_pos {
            if let Some(c) = self.build_inline_comments_between_doc_opt(cursor, fp) {
                parts.push(c);
            }
            if cursor > decl.span.start {
                parts.push(d.text(" "));
            }
            parts.push(d.text("function"));
            cursor = fp + "function".len() as u32;
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

        // Everything after the keyword→name gap is collected into `tail`, so a
        // *line* comment in that gap can indent the whole continuation one level
        // (uniform declaration-header rule). Block/no-comment cases stay inline.
        let mut tail: DocBuf = DocBuf::new();
        let name_start = if let Some(id) = &decl.id {
            tail.push(self.identifier_name_doc(id));

            // Comments between name and type params/parens: `function fn1/* c */ <T>()` or `fn1 /* c */()`
            // Line comments get a hardline to prevent absorbing type params as comment text
            let comment_end = decl
                .type_parameters
                .as_ref()
                .map_or(decl.params_start, |tp| tp.span.start);
            tail.push(self.build_name_to_type_params_comments(
                id.span.end,
                comment_end,
                CommentSpacing::for_type_params(decl.type_parameters.is_some()),
            ));
            id.span.start
        } else {
            // Anonymous function (export default): the gap runs to the params/type-params.
            decl.type_parameters
                .as_ref()
                .map_or(decl.params_start, |tp| tp.span.start)
        };

        // Type parameters (TypeScript generics): function foo<T>()
        if let Some(type_params) = &decl.type_parameters {
            tail.push(self.build_type_parameter_declaration_doc_wrapping(type_params));

            // Comments between type_params `>` and `(` go after type_params
            if let Some(pp) = find_char_skipping_comments(
                self.source.as_bytes(),
                type_params.span.end as usize,
                self.source.len(),
                b'(',
            ) {
                self.append_type_params_to_paren_comments(
                    &mut tail,
                    type_params.span.end,
                    pp as u32,
                );
            }
        }

        // Signature (params + return type) in a single group
        let (sig_doc, sig_end) = self.build_callable_signature_doc(
            decl.params,
            decl.type_parameters.as_ref(),
            decl.return_type.as_ref(),
            decl.params_start,
            decl.body.span.start,
        );
        tail.push(sig_doc);

        // Handle comments between signature and body: function a() /* comment */ {}
        self.append_body_with_sig_comments(&mut tail, sig_end, &decl.body);

        if decl.id.is_some() {
            // Named: a line comment in the `function`/`*`→name gap indents the
            // whole continuation. `export default function` (anonymous) keeps the
            // keyword→params gap flat below.
            let continuation = d.concat(&tail);
            parts.push(self.build_keyword_to_name_continuation(cursor, name_start, continuation));
        } else {
            // Anonymous function (export default): keyword→params gap stays flat.
            // Line comments get hardline to prevent absorbing parens: `function // c\n()`
            parts.push(self.build_keyword_to_name_comments(cursor, name_start));
            parts.extend(tail);
        }

        d.concat(&parts)
    }
}
