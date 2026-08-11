// Function declaration printing for TypeScript

use super::super::types::function_types::group_params_if_should;
use super::Printer;
use crate::ast::internal;
use crate::printer::CommentSpacing;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

/// The modifier keywords that open a function head, in source order.
///
/// `declare` and `async` **can co-occur**: `declare async function f(): Promise<void>;`
/// is one ambient signature to tsc's parser (`[DeclareKeyword, AsyncKeyword]`, no
/// `parseDiagnostics`), its TS1040 prohibition being a checker grammar error tsv defers.
/// So this is a keyword *sequence*, not a slot — collapsing it to one keyword printed
/// `async function` and DROPPED `declare`, since the pair-selecting arm answered with
/// whichever flag it tested first.
///
/// [`Printer::push_function_keyword_head`] owns every keyword here rather than leaving
/// the caller to print one and orphan the gap behind it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionHeadModifier {
    None,
    Async,
    Declare,
    DeclareAsync,
}

impl FunctionHeadModifier {
    /// The keywords' source text in source order — empty for a bare `function`.
    fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::None => &[],
            Self::Async => &["async"],
            Self::Declare => &["declare"],
            Self::DeclareAsync => &["declare", "async"],
        }
    }

    /// The modifier an `async` flag selects — the shape three of the four callers have,
    /// since only an ambient `TSDeclareFunction` can carry `declare` at all.
    pub(crate) fn from_async(is_async: bool) -> Self {
        Self::from_flags(is_async, false)
    }

    /// The modifier a node carrying BOTH flags selects.
    pub(crate) fn from_flags(is_async: bool, is_declare: bool) -> Self {
        match (is_async, is_declare) {
            (true, true) => Self::DeclareAsync,
            (true, false) => Self::Async,
            (false, true) => Self::Declare,
            (false, false) => Self::None,
        }
    }
}

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
        let (params_doc, return_type_doc, sig_end) = self.build_signature_params_return(
            params,
            type_parameters,
            return_type,
            params_start,
            body_start,
        );

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

    /// Emit a function-like head — the opening modifier (`async` / `declare`, when
    /// present), the gap between it and the `function` keyword, `function` itself, and a
    /// generator `*` — returning the source position just past what was emitted, where
    /// the keyword→name gap begins.
    ///
    /// Shared by the function **declaration**, the function **expression** and the
    /// bodiless **overload signature**, which used to answer the modifier→`function` gap
    /// three ways: the declaration emitted it (preserving the author's position,
    /// `async /* c */ function f() {}`) while the other two pushed a bare `"async "` and
    /// let the gap's comments fall through to whichever emitter came next — the
    /// keyword→name gap for a named function, the parameter list for an anonymous one —
    /// relocating them **across the `function` keyword**. Prettier relocates here too,
    /// and inconsistently (into the body for an anonymous function, after the keyword for
    /// a named one), so it is no oracle; the declaration's answer is tsv's, and now there
    /// is one of it. See `docs/conformance_prettier_ts_comments.md` §Comment relocation.
    ///
    /// The `function` keyword is **found**, never computed from the modifier's end: a
    /// comment may sit between the two, so the keyword's offset is not a constant away.
    /// (The modifier's own end is arithmetic, which is sound because it opens the span.)
    /// Returning the cursor is what keeps the caller's next gap from re-claiming this
    /// one — reading it as `span.start..name.start` is exactly how the expression form
    /// printed the comment twice.
    pub(crate) fn push_function_keyword_head(
        &self,
        parts: &mut DocBuf,
        span_start: u32,
        search_end: u32,
        modifier: FunctionHeadModifier,
        is_generator: bool,
    ) -> u32 {
        let d = self.d();
        let mut cursor = span_start;

        // The head's modifier keywords, in source order. The FIRST opens the node's span,
        // which is what makes its arithmetic sound; each later one is located by search,
        // since only the first has a guaranteed position. Every one must be emitted HERE
        // rather than by the caller: the gap behind each belongs to the emitter below, and
        // a caller that printed a modifier itself left that gap claimed by nobody — a DROP.
        let modifier_keywords = modifier.keywords();
        for (i, word) in modifier_keywords.iter().enumerate() {
            if i == 0 {
                parts.push(d.text(word));
                cursor = span_start + word.len() as u32;
            } else {
                // The `declare`→`async` gap, through the same line-comment-SAFE emitter
                // the `async`→`function` gap uses below.
                let pos = self
                    .find_keyword_in_range(cursor, search_end, word)
                    .unwrap_or(cursor);
                parts.push(self.build_keyword_to_name_comments(cursor, pos));
                parts.push(d.text(word));
                cursor = pos + word.len() as u32;
            }
        }

        // Find "function" in source after cursor, skipping comments
        let function_pos = self.find_keyword_in_range(cursor, search_end, "function");
        if !modifier_keywords.is_empty() {
            // The `async`→`function` gap, through the line-comment-SAFE emitter (it
            // returns the bare separating space when the gap is empty). An inline
            // emitter here swallowed the whole declaration head onto a `//`'s line —
            // reachable because the statement parser used to weld `async⏎function f() {}`
            // into one async function instead of splitting it per `async [no
            // LineTerminator here] function`. Both halves are fixed; the emitter stays
            // the safe one rather than resting on the parser's guarantee.
            parts.push(self.build_keyword_to_name_comments(cursor, function_pos.unwrap_or(cursor)));
        }
        parts.push(d.text("function"));
        if let Some(fp) = function_pos {
            cursor = fp + "function".len() as u32;
        }

        if is_generator {
            // The `*` is emitted, but the cursor deliberately does NOT step over it: the
            // gap that may hold a comment is `function`→`*`, and leaving it inside the
            // caller's following range is what gets it printed — after the `*`, which is
            // where the spaced spelling already put it and where prettier puts it in a
            // declaration.
            //
            // Advancing the cursor here is what dropped the comment: `cursor + 1` assumed
            // the `*` was adjacent, so with a comment between the two the cursor landed
            // INSIDE the comment and the caller's range began past the comment's own
            // start, where the emitter skips it (`function/* c */*g() {}` →
            // `function* g() {}`). The spaced spelling survived only because one byte
            // happened to stop short of the comment.
            //
            // Emitting the gap here instead — the other repair — is worse than the bug:
            // this is an inline position, so a `//` printed here swallows the name and
            // the parameters onto its own line, trading a lost comment for lost CODE. The
            // caller's emitter is the line-comment-safe one, so the region belongs to it.
            parts.push(d.text("*"));
        }

        cursor
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
        let cursor = self.push_function_keyword_head(
            &mut parts,
            decl.span.start,
            search_end,
            FunctionHeadModifier::from_async(decl.r#async),
            decl.generator,
        );

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
            self.push_name_to_type_params_comments(
                &mut tail,
                id.span.end,
                comment_end,
                CommentSpacing::for_type_params(decl.type_parameters.is_some()),
            );
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
