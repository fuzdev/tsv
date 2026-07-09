// Type-argument instantiation (`<T, U>`) rendering

use super::Printer;
use super::helpers::{is_simple_type_arg, should_hug_union_type, unwrap_parenthesized};
use crate::ast::internal::{self, TSType};
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build doc for a type used as a type argument.
    ///
    /// For single type arg contexts, uses normal doc (allows object types to break).
    /// For multiple type arg contexts, uses hugging (objects don't break independently).
    pub(in crate::printer) fn build_type_arg_doc(
        &self,
        param: &TSType<'_>,
        is_multi_arg: bool,
    ) -> DocId {
        if is_multi_arg {
            self.build_type_doc_for_type_arg(param)
        } else {
            self.build_type_doc(param)
        }
    }

    /// Comments that force the `<...>` list to the multiline layout: line
    /// comments anywhere (including before the first argument, e.g.
    /// `Foo<// leading\n  a>`) or own-line block comments — neither can render
    /// inline.
    fn type_arguments_force_expansion(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
    ) -> bool {
        // Zero-comment window gate: one binary search over the whole `<…>` span.
        // Every sub-query below is bounded within `[args.span.start, args.span.end]`,
        // and `has_comments_between` only yields comments fully inside its range — so
        // when no comment lies inside the `<…>`, all three are provably false. Skips
        // them (and their delimited-list trivia scans) on the overwhelmingly common
        // comment-free type argument (`Foo<T>`, `Map<K, V>`, …).
        if !self.has_comments_between(args.span.start, args.span.end) {
            return false;
        }
        let has_leading_line_comment = args.params.first().is_some_and(|first| {
            self.has_line_comments_between(args.span.start + 1, first.span().start)
        });
        has_leading_line_comment
            || self.has_line_comments_in_delimited_list(
                args.params,
                TSType::span,
                args.span.end - 1,
            )
            || self.has_own_line_block_comments_in_bracket_list(
                args.span,
                args.params,
                TSType::span,
            )
    }

    /// Build doc for type arguments: `<T, U>`.
    ///
    /// Single arg: always inline. Multi-arg: group-based breaking via shared helper.
    /// Use `build_type_arguments_doc_wrapping` for single-arg hugging (e.g., `Array<{...}>`).
    pub(crate) fn build_type_arguments_doc(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        let d = self.d();
        if args.params.is_empty() {
            return d.text("<>");
        }

        if self.type_arguments_force_expansion(args) {
            return self.build_type_arguments_doc_with_line_comments(args);
        }

        // Single type argument: inline (matches Prettier's shouldInline for len==1)
        if args.params.len() == 1 {
            let mut parts = DocBuf::new();
            let prev_end = args.span.start + 1; // After the opening `<`
            let param_start = args.params[0].span().start;
            let param_end = args.params[0].span().end;
            let before_close = args.span.end - 1;

            self.append_leading_inline_block_comments(&mut parts, prev_end, param_start);
            parts.push(self.build_type_arg_doc(&args.params[0], false));
            self.append_trailing_inline_block_comments(&mut parts, param_end, before_close);
            return d.concat(&[d.text("<"), d.concat(&parts), d.text(">")]);
        }

        // Multiple type arguments: use group so they can break at print width.
        // Matches Prettier's group([<, indent([softline, join([",", line], args)]), softline, >])
        self.build_type_arguments_doc_multi_arg(args)
    }

    /// Build doc for type arguments with width-based wrapping support.
    ///
    /// Inline: `<T, U, V>`
    /// Wrapped: `<\n\tT,\n\tU,\n\tV\n>`
    ///
    /// Special case: single TypeLiteral argument hugs the opening `<`:
    /// `Array<{prop: string}>` stays hugged, and when broken:
    /// ```text
    /// Array<{
    ///     prop: string;
    /// }>
    /// ```
    ///
    /// Use this when type arguments should break independently of parent context,
    /// such as in property type annotations.
    pub(crate) fn build_type_arguments_doc_wrapping(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        let d = self.d();
        if args.params.is_empty() {
            return d.text("<>");
        }

        if self.type_arguments_force_expansion(args) {
            return self.build_type_arguments_doc_with_line_comments(args);
        }

        // Single type argument inlining, matching Prettier's `shouldInline` logic.
        // Three categories are inlined (no group/softlines):
        //
        // 1. Simple types (`is_simple_type_arg`): keywords, literals, `this`, and a
        //    bare TypeReference without type args. Atomic — never need breaking.
        // 2. Object types: TypeLiteral and Mapped types handle their own breaking.
        // 3. Hugged unions: unions with a brace-delimited member like `{...} | null`.
        //
        // Without inlining, the group/softlines create Break-mode Line nodes in
        // `fits()` rest_commands, causing upstream groups (like arrays in Fluid
        // assignment layout) to incorrectly appear to "fit" — Line in Break mode
        // returns true from `fits()`, short-circuiting the width check.
        if args.params.len() == 1 {
            let unwrapped = unwrap_parenthesized(&args.params[0]);
            let is_huggable = is_simple_type_arg(&args.params[0])
                || matches!(unwrapped, TSType::TypeLiteral(_) | TSType::Mapped(_))
                || matches!(unwrapped, TSType::Union(u) if
                    should_hug_union_type(u)
                    && u.types.iter().any(|t| matches!(t, TSType::TypeLiteral(_) | TSType::Mapped(_)))
                );
            if is_huggable {
                let mut parts = smallvec![d.text("<")];

                // Include leading comments: `Array</* comment */ {...}>`
                let param_start = args.params[0].span().start;
                let param_end = args.params[0].span().end;
                let after_open = args.span.start + 1; // After the opening `<`
                let before_close = args.span.end - 1; // Before the closing `>`
                self.append_leading_inline_block_comments(&mut parts, after_open, param_start);

                parts.push(self.build_type_arg_doc(&args.params[0], false));

                // Include trailing comments: `Array<{...} /* trailing */>`
                self.append_trailing_inline_block_comments(&mut parts, param_end, before_close);

                parts.push(d.text(">"));
                return d.concat(&parts);
            }
        }

        self.build_type_arguments_doc_multi_arg(args)
    }

    /// Build multi-arg type arguments with group-based breaking.
    ///
    /// Matches Prettier's `group([<, indent([softline, join([",", line], args)]), softline, >])`.
    /// Used by both `build_type_arguments_doc` and `build_type_arguments_doc_wrapping`
    /// for 2+ type arguments (and non-huggable single args in the wrapping variant).
    fn build_type_arguments_doc_multi_arg(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = DocBuf::new();
        let mut prev_end = args.span.start + 1; // After the opening `<`

        for (i, param) in args.params.iter().enumerate() {
            let param_start = param.span().start;
            let is_last = i == args.params.len() - 1;

            let mut arg_parts = DocBuf::new();

            // Add leading block comments before this type argument
            self.append_leading_inline_block_comments(&mut arg_parts, prev_end, param_start);

            arg_parts.push(self.build_type_arg_doc(param, true));

            // Add trailing block comments after this type argument (before comma)
            let param_end = param.span().end;
            prev_end = if i + 1 < args.params.len() {
                let next_start = args.params[i + 1].span().start;
                let comma_pos = self.find_list_comma(param_end, next_start);
                self.append_trailing_inline_block_comments(&mut arg_parts, param_end, comma_pos);
                comma_pos + 1 // After comma — leading comments picked up next iteration
            } else {
                let before_close = args.span.end - 1;
                self.append_trailing_inline_block_comments(&mut arg_parts, param_end, before_close);
                before_close
            };

            if i > 0 {
                inner_parts.push(d.line());
            }
            inner_parts.push(d.concat(&arg_parts));
            if !is_last {
                inner_parts.push(d.text(","));
            }
            // Note: type arguments don't get trailing commas (unlike params)
        }

        d.group(d.concat(&[
            d.text("<"),
            d.indent_softline(d.concat(&inner_parts)),
            d.softline(),
            d.text(">"),
        ]))
    }

    /// Build doc for type arguments with expanding comments (line or own-line block).
    ///
    /// Line comments and own-line block comments force multiline because they can't appear inline.
    fn build_type_arguments_doc_with_line_comments(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        // Type-position type arguments render each argument with `build_type_arg_doc`;
        // the layout is shared with call/`new`-expression arguments.
        self.build_angle_list_with_line_comments(args, true)
    }
}
