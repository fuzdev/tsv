// Object expression printing for TypeScript
//
// Handles printing of object expressions with:
// - Width-based wrapping via doc-builder
// - Comment preservation (block and line comments)
// - Property shorthand detection
// - String key normalization (unquote valid identifiers)
// - Blank line preservation between properties

use crate::ast::internal::{self, Expression, Literal, LiteralValue};
use crate::printer::CommentSpacing;
use crate::printer::expressions::assignment::RhsCommentInfo;
use crate::printer::expressions::literals::is_valid_js_identifier;
use crate::printer::layout::hang_after_operator;
use crate::printer::{CommentVec, Printer, StandaloneGlue};
use smallvec::{SmallVec, smallvec};
use tsv_lang::Span;
use tsv_lang::TAB_WIDTH;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::visual_width;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Build a Doc for an object expression
    ///
    /// Handles comments between properties, blank line preservation, and trailing comments.
    pub(in crate::printer) fn build_object_doc(
        &self,
        obj: &internal::ObjectExpression<'_>,
    ) -> DocId {
        let d = self.d();
        // Check for comments inside the object.
        //
        // **on page**: this is the object-wide fast gate, and it short-circuits the
        // *layout* work too — a `false` here routes every property to the plain
        // `key: value` form. An owned annotation on a value is on the page and hangs the
        // value onto a continuation line exactly as any other own-line comment does, so
        // the gate has to see it. (`build_object_doc_expanded` carries the twin gate.)
        let has_comments = self.has_comments_on_page_between(obj.span.start, obj.span.end);

        // Check if object contains line comments or block comments on their own line (force multiline)
        let has_line_comments = self.has_line_comments_between(obj.span.start, obj.span.end);

        // Check for block comments on their own line (not same line as any property).
        // Only relevant when the object has comments at all — otherwise there are no
        // block comments to be standalone, so skip the per-property span collection
        // (the common comment-free object pays nothing). The glue half is the SOURCE
        // reading: the object's own comma is re-emitted structure outside every property
        // span, so a comment it follows is not standalone — see `StandaloneGlue` for why
        // the type literal answers this differently.
        let has_standalone_block_comment = has_comments && {
            let property_spans: SmallVec<[_; 8]> = obj
                .properties
                .iter()
                .map(internal::ObjectProperty::span)
                .collect();
            self.has_standalone_block_comment(
                obj.span.start,
                obj.span.end,
                &property_spans,
                StandaloneGlue::Source,
            )
        };

        if obj.properties.is_empty() {
            // Handle empty object with comments
            return self.build_empty_braces_inline_with_comments_doc(obj.span);
        }

        // Check if source has newline after opening brace
        let first_prop_start = obj.properties[0].span().start;
        let has_source_newline = self.has_newline_between(obj.span.start + 1, first_prop_start);

        // Check if any property value has multiline content (e.g., line continuation strings)
        // Prettier expands objects containing multiline strings (recursively)
        let has_multiline =
            crate::printer::container_may_have_multiline_content(obj.span, self.source)
                && obj.properties.iter().any(|prop| match prop {
                    internal::ObjectProperty::Property(p) => {
                        crate::printer::has_multiline_content(&p.value, self.source)
                    }
                    internal::ObjectProperty::SpreadElement(s) => {
                        crate::printer::has_multiline_content(s.argument, self.source)
                    }
                });

        // Decide the formatting strategy
        // must_break: conditions that require hardlines (comments, multiline content)
        // has_source_newline: prefers expanded, but uses group_break for proper propagation
        let must_break = has_line_comments || has_standalone_block_comment || has_multiline;

        if has_comments || must_break {
            // Comment-aware path
            // Use hardlines when must_break, use line() when only has_comments
            // This allows inline objects with block comments to stay inline if they fit
            let mut parts = d.pooled_docbuf();
            let mut prev_end = obj.span.start + 1; // After opening brace

            // A comment trailing the opening `{` is kept on the `{` line when the
            // object expands — both a line comment and a block comment before a
            // first property on a later line (divergence from prettier, which
            // relocates it to its own line as the first property's leading
            // comment). See conformance_prettier_ts_comments.md §Comment relocation (Object
            // literal `{`).
            let (brace_line_prefix, brace_pull_pos) =
                self.delimiter_line_comment_prefix_object(obj.span.start, first_prop_start);
            // The prefix is only emitted on the break path, so a fired pull forces
            // must-break (an expanding object with a block on the `{` line breaks
            // via has_source_newline, not the must_break conditions above).
            let must_break = must_break || brace_pull_pos.is_some();

            for (i, prop) in obj.properties.iter().enumerate() {
                let prop_start = prop.span().start;
                let is_first = i == 0;

                // The rest of the gap, resuming where the previous property's trailing run
                // stopped — the element-comma partition (see `collect_item_leading_comments`).
                let comments = self.collect_item_leading_comments(
                    prev_end,
                    prop_start,
                    is_first.then_some(brace_pull_pos).flatten(),
                );

                // For non-first properties, add separator. An author BLANK line takes the
                // hardline separator even when nothing else forces the break: a soft
                // `line` cannot carry a blank, so routing a blank gap through `line()`
                // DROPS it — and the comment-free path below preserves the identical
                // authoring (`{ a,⏎⏎b }`), so a commented object dropping it was the two
                // paths disagreeing about one gap. Preserving it forces the object open,
                // which is what the comment-free path's `literalline` does too.
                if !is_first {
                    if must_break || self.item_gap_has_blank_line(prev_end, prop_start) {
                        // Must break: check for blank line preservation
                        self.push_item_blank_separator(&mut parts, prev_end, prop_start);
                    } else {
                        // May stay inline: use line() for group-based breaking
                        parts.push(d.line());
                    }
                }

                // Print the leading-comment run on the shared rule (prettier's
                // `printLeadingComment`), the one the block / class / interface / member
                // lists use: the separator after each comment is keyed on the source around
                // *that comment*, never on where the property starts. Keying it on the
                // property split a run the author glued (`/* c1 */ /* c2 */⏎x: 1`) — the bug
                // family docs/comments.md §"Leading comments" names. The emitter owns the
                // blank-line preservation this loop used to hand-roll on both sides
                // (between two comments, and between the last one and the property), and
                // emits a soft `line` where the run may still collapse, so an object that
                // stays inline is decided by its group rather than by a hardcoded space.
                // TODO: the object literal is a third case in the array-family / params-family
                // split (docs/comments.md §Array family vs params family) and it is not named
                // there. It gives a property no group of its own, so a leading run's soft
                // `line` breaks — `p1: 1,⏎/* c */⏎p2: 2` — where the ARRAY family collapses it
                // onto the element (`'aaaa',⏎/* c */ 'bbbb'`) from the identical authoring.
                // Prettier relocates the block before the comma at both sites (the sanctioned
                // §Array element end-of-line block comment rule, currently cataloged for the
                // array only), so its own grouping is not observable here and neither form is
                // validated against it. Settle which family the object literal belongs to
                // before cataloging the object / specifier / enum face of that divergence —
                // sanctioning the own-line form first would pin whichever one this is.
                self.push_leading_comments_before(&mut parts, &comments, prop_start);

                // Build property doc — a preceding format-ignore directive keeps the
                // property's source verbatim (trailing comment/comma handled normally).
                // Same window as the leading scan: a directive trailing the previous
                // property is inert by the placement floor (`is_honored_directive`), not
                // by where the window starts.
                // Resolved once as the SLICE rather than a bool, because the trailing
                // seam below needs the same answer (`Printer::element_claim_anchor`).
                let frozen_span = self
                    .member_gap_frozen(prev_end, prop_start)
                    .then(|| prop.span());
                parts.push(match frozen_span {
                    Some(slice) => self.raw_source_doc(slice),
                    None => self.build_object_property_doc(prop, has_comments),
                });

                // Trailing comments around the separator comma — block comments
                // before the comma, the comma, an after-comma block on the last
                // property preserved in place (`trailingComma: 'none'`), then line
                // comments as a suffix. Shared with the destructuring-pattern
                // builders via `collect_trailing_comments` /
                // `push_element_comma_trailing`.
                // Under a freeze the verbatim slice has already printed the property's
                // stripped-paren interior, so the seam must start at the END of that
                // slice — `value_end()` is inside it, and re-claiming a comment already
                // on the page prints it twice, then three times, then four: the emitted
                // form still carries the directive, so it re-freezes and the run grows
                // on every pass.
                let prop_end = Self::element_claim_anchor(frozen_span, prop.value_end());
                let upper_bound = obj
                    .properties
                    .get(i + 1)
                    .map_or(obj.span.end, |next| next.span().start);

                let is_last = i == obj.properties.len() - 1;
                let spread = prop.as_spread();
                let mut trailing = self.collect_trailing_comments(prop_end, upper_bound, is_last);
                // A spread whose stripped parens held a `//` already ends its line in one;
                // a second may not weld onto it.
                trailing.demote_line_after_deferred(
                    spread.is_some_and(|s| self.spread_element_defers_trailing_line_comment(s)),
                );
                let comma = if is_last { d.empty() } else { d.text(",") };
                self.push_element_comma_trailing(&mut parts, &trailing, comma);

                // The object's share of a spread's stripped-paren interior: the own-line
                // blocks the spread's own doc leaves behind, each a sibling line the
                // object cannot stay collapsed around. Emitted past the comma, like the
                // array element loop and the argument-list gaps.
                if let Some(s) = spread {
                    self.push_spread_element_own_line_block_comments(&mut parts, s);
                }

                prev_end = trailing.end_pos;
            }

            // Trailing comments before the closing brace, through the shared end-of-body
            // run (`Printer::build_trailing_body_comments_doc`) rather than a copy of it:
            // the same walk, the same "already trailed by the last property" question, and
            // the same stripped-shell blank anchor every other container gets. The copy
            // this replaces had drifted from it on all three. An object that may still
            // COLLAPSE takes a soft `line` separator its group decides; a broken one takes
            // the hardline, like every other body.
            let closing_brace_pos = obj.span.end - 1;
            let separator = if must_break { d.hardline() } else { d.line() };
            parts.extend(self.build_trailing_closer_comments_doc(
                prev_end,
                closing_brace_pos,
                false,
                separator,
            ));

            if must_break {
                // Forced multiline - use hardlines for predictable formatting
                let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
                let (indented_content, closing_line) =
                    self.wrap_with_decl_indent(inner, d.hardline());

                d.concat(&[
                    d.text("{"),
                    d.concat(&brace_line_prefix),
                    indented_content,
                    closing_line,
                    d.text("}"),
                ])
            } else {
                // May stay inline - use group with bracketSpacing boundaries for
                // width-based breaking: a space when flat (`{ foo }`), a newline when
                // it breaks (brace_line_prefix is empty here — pulling implies must_break).
                let inner = d.concat(&[d.line(), d.concat(&parts)]);
                let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.line());

                self.wrap_object_braces(indented_content, closing_line, has_source_newline)
            }
        } else {
            // No comments, no forced multiline: use width-based wrapping with soft lines
            let mut parts = d.pooled_docbuf();

            // Whether each gap between consecutive properties carries an author blank line.
            // Derived ONCE per gap: the loop below asks the same question from both sides —
            // "is there a blank before me?" at property `i`, and "before the next one?" when
            // emitting `i`'s separator — which is the same gap, and computing it twice invites
            // the two answers to drift apart.
            //
            // Through the shared predicate rather than a raw `is_next_line_empty`, for the
            // stripped-shell anchor it carries (`Printer::item_gap_has_blank_line`): a value's
            // erased grouping parens are not an author blank (`{ k: (1⏎⏎), b }`), and a copy
            // of this scan is how the comment-bearing path above and this one came to answer
            // that differently. With no comment in the gap the bound is the next property
            // either way, so the two spellings agree by construction.
            let gap_blank: SmallVec<[bool; 8]> = obj
                .properties
                .windows(2)
                .map(|pair| self.item_gap_has_blank_line(pair[0].value_end(), pair[1].span().start))
                .collect();

            for (i, prop) in obj.properties.iter().enumerate() {
                // Check for blank line before this property (preserved in multiline).
                let has_blank_before = i > 0 && gap_blank[i - 1];

                if has_blank_before {
                    // Blank line preservation
                    parts.push(d.literalline());
                    parts.push(d.hardline());
                }

                // Build property doc
                let prop_doc = self.build_object_property_doc(prop, has_comments);
                parts.push(prop_doc);

                // Add comma and line break
                if i < obj.properties.len() - 1 {
                    parts.push(d.text(","));
                    // Only add a line break when the next property has no blank line before
                    // it — the blank's own `literalline` + `hardline` already separates them.
                    if !gap_blank[i] {
                        parts.push(d.line());
                    }
                }
                // No trailing comma on the last property (trailingComma: 'none').
            }

            // Width-based wrapping: bracketSpacing boundaries (space when flat
            // `{ foo }`, newline when broken).
            let inner = d.concat(&[d.line(), d.concat(&parts)]);
            let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.line());

            self.wrap_object_braces(indented_content, closing_line, has_source_newline)
        }
    }

    /// Wrap content in braces with appropriate grouping for object expressions.
    ///
    /// Uses `group_break` when source had newlines (propagates break upward),
    /// otherwise uses `group` for width-based breaking.
    fn wrap_object_braces(
        &self,
        indented_content: DocId,
        closing_line: DocId,
        has_source_newline: bool,
    ) -> DocId {
        let d = self.d();
        let object_doc = d.concat(&[d.text("{"), indented_content, closing_line, d.text("}")]);
        if has_source_newline {
            d.group_break(object_doc)
        } else {
            d.group(object_doc)
        }
    }

    /// Build a Doc for an object expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the object to expand internally
    /// with hardlines so fits() can correctly measure the first line.
    /// Produces: `{\n  prop,\n}` with actual hardlines.
    pub(in crate::printer) fn build_object_doc_expanded(
        &self,
        obj: &internal::ObjectExpression<'_>,
    ) -> DocId {
        let d = self.d();
        // A commented object hands off to `build_object_doc` wholesale: the loop below
        // emits property docs only, so every structural comment — the `{`→first-property
        // gap, the inter-property gaps, the trailing gap before `}`, and a dangling
        // comment in an empty `{}` — would be DROPPED here (content loss, not
        // relocation), which is why the gate also precedes the empty-object arm. The
        // forced-hardline form the caller wants for its `fits()` measurement is only ever
        // needed on the comment-free path, and that is exactly where the two agree.
        // **on page**, in lockstep with the twin gate in `build_object_doc`.
        if self.has_comments_on_page_between(obj.span.start, obj.span.end) {
            return self.build_object_doc(obj);
        }
        if obj.properties.is_empty() {
            return d.text("{}");
        }

        let mut parts: DocBuf = DocBuf::new();
        for (i, prop) in obj.properties.iter().enumerate() {
            // Comment-free past the gate, so the per-property comment queries are dead.
            let prop_doc = self.build_object_property_doc(prop, false);
            parts.push(prop_doc);

            if i < obj.properties.len() - 1 {
                parts.push(d.text(","));
                parts.push(d.hardline());
            }
            // No trailing comma on the last property under `trailingComma: 'none'`.
        }

        d.concat(&[
            d.text("{"),
            d.indent_hardline(d.concat(&parts)),
            d.hardline(),
            d.text("}"),
        ])
    }

    /// Build a Doc for an object property (either Property or SpreadElement)
    ///
    /// `has_comments` is the object-wide comment-presence flag (one binary search
    /// over the whole `{…}` span); it gates the per-property key→value comment
    /// queries in `build_property_doc`.
    fn build_object_property_doc(
        &self,
        prop: &internal::ObjectProperty<'_>,
        has_comments: bool,
    ) -> DocId {
        match prop {
            internal::ObjectProperty::Property(p) => self.build_property_doc(p, has_comments),
            internal::ObjectProperty::SpreadElement(s) => self.build_spread_doc(s),
        }
    }

    /// Build a Doc for a single property
    ///
    /// `has_comments` is the object-wide comment-presence flag: when it is false,
    /// no comment lies anywhere in the object span, so none can lie in this
    /// property's key→value gap either — the colon scan and the per-gap comment
    /// lookups are skipped (canonical reference: `build_params_doc_with_comments`).
    fn build_property_doc(&self, prop: &internal::Property<'_>, has_comments: bool) -> DocId {
        let d = self.d();
        // For computed keys, use expression doc (preserves string quotes)
        // For regular keys, use property key doc (converts strings to bare identifiers when valid)
        // Track where comments after the key region end (after `]` for computed, after key for normal)
        let key_region_end;
        let key_doc = if prop.computed {
            // Assignment expressions need parens in computed keys: {[(a = b)]: c}
            let key_expr_doc = self.build_computed_key_expr_doc(&prop.key);
            let (doc, end) =
                self.build_computed_key_bracket_doc(prop.span.start, &prop.key, key_expr_doc);
            key_region_end = end;
            doc
        } else {
            key_region_end = prop.key.span().end;
            self.build_property_key_doc(&prop.key)
        };

        // Add getter/setter prefix if applicable, preserving comments between
        // keyword and name (e.g., `get /* c */ a()`)
        let key_doc = match prop.kind {
            internal::PropertyKind::Get | internal::PropertyKind::Set => {
                let kind_text = if matches!(prop.kind, internal::PropertyKind::Get) {
                    "get "
                } else {
                    "set "
                };
                let mut kw_parts = DocBuf::new();
                self.push_accessor_keyword_doc(
                    &mut kw_parts,
                    kind_text,
                    prop.span.start,
                    prop.key.span().start,
                    prop.computed,
                );
                kw_parts.push(key_doc);
                d.concat(&kw_parts)
            }
            internal::PropertyKind::Init => key_doc,
        };

        // Handle getter/setter vs method vs regular property
        if matches!(
            prop.kind,
            internal::PropertyKind::Get | internal::PropertyKind::Set
        ) {
            // Getter/setter: `get x() {}` or `set x(v) {}`
            if let Expression::FunctionExpression(func) = &prop.value {
                let func_doc = self.build_function_doc_body(func);
                // Comments between key and params: get [x] /* c */() {}
                // Line comments get a hardline to prevent absorbing parens as comment text
                let params_start = func.params_start;
                match self.build_name_to_type_params_comments_opt(
                    key_region_end,
                    params_start,
                    CommentSpacing::Leading,
                ) {
                    Some(comments) => d.concat(&[key_doc, comments, func_doc]),
                    None => d.concat(&[key_doc, func_doc]),
                }
            } else {
                key_doc
            }
        } else if prop.method {
            // Method shorthand: `foo() {}`, `async foo() {}`, `*gen() {}`, or `async *gen() {}`
            if let Expression::FunctionExpression(func) = &prop.value {
                let func_doc = self.build_function_doc_body(func);
                // Build prefix: async? + *?, preserving comments after `async`
                // (e.g., `async /* c */ m()`)
                let key_start = prop.key.span().start;
                let mut parts = DocBuf::new();
                let mut cursor = prop.span.start;
                if func.r#async {
                    self.push_member_keyword_doc(&mut parts, "async ", &mut cursor, key_start);
                }
                if func.generator {
                    self.push_generator_star_doc(&mut parts, cursor, key_start, prop.computed);
                } else if func.r#async {
                    // Comments before the name (bounded at `[` for computed keys,
                    // whose inner comments the bracket builder handles)
                    let bound = self.computed_key_name_bound(cursor, key_start, prop.computed);
                    self.push_pre_name_comments_doc(&mut parts, cursor, bound);
                }
                parts.push(key_doc);

                // Handle comments between method name and type params/parameters: foo /* comment */ ()
                // Use key_region_end (after `]` for computed) to avoid re-finding bracket comments
                // Stop at type_params start when present — comments between `>` and `(`
                // are handled by build_function_expression_signature_doc
                // Line comments get a hardline to prevent absorbing type params as comment text
                let comment_search_end = func
                    .type_parameters
                    .as_ref()
                    .map_or(func.params_start, |tp| tp.span.start);
                self.push_name_to_type_params_comments(
                    &mut parts,
                    key_region_end,
                    comment_search_end,
                    CommentSpacing::for_type_params(func.type_parameters.is_some()),
                );

                parts.push(func_doc);
                d.concat(&parts)
            } else {
                // Fallback for malformed AST
                let value_doc = self.build_expression_doc(&prop.value);
                d.concat(&[key_doc, d.text(": "), value_doc])
            }
        } else if prop.shorthand {
            // Shorthand with an initializer (`{a = 1}`) — a `CoverInitializedName`, whose
            // early error tsv defers, so the value is an `AssignmentExpression` spanning the
            // whole `a = 1` (an `AssignmentPattern` where a pattern was refined in place).
            //
            // ⚠️ Printed by the VALUE's own doc, not reassembled from `key + " = " + right`:
            // that spelling emits no name→`=` gap, so a comment the author wrote there
            // (`{a /* c */ = 1}`) had no emitter at all. It survived only because the
            // element-comma seam, anchored at the KEY's end, claimed it and printed it past
            // the initializer — a relocation across the binding, and a DROP the moment the
            // anchor was corrected. The value's doc prints the key and that gap alike, and
            // is not parenthesized here: the shorthand form is the syntax.
            match &prop.value {
                Expression::AssignmentExpression(_) | Expression::AssignmentPattern(_) => {
                    self.build_expression_doc(&prop.value)
                }
                _ => key_doc,
            }
        } else {
            // Regular property.
            //
            // Zero-comment fast gate: when the object has no comments at all, none
            // can lie between this key and its value, so skip the colon scan and the
            // per-gap comment lookups and emit the plain `key: value` layout directly
            // (canonical reference: build_params_doc_with_comments).
            if !has_comments {
                let needs_parens =
                    self.needs_parens(&prop.value, super::ParenContext::ObjectPropertyValue);
                return if needs_parens {
                    let value_doc = d.concat(&[
                        d.text("("),
                        self.build_expression_doc(&prop.value),
                        d.text(")"),
                    ]);
                    d.concat(&[key_doc, d.text(": "), value_doc])
                } else {
                    let is_short_key = self.is_short_property_key(&prop.key, prop.computed);
                    // A comment-free object provably holds no directive either.
                    self.build_assignment_layout(
                        key_doc,
                        ":",
                        &prop.value,
                        is_short_key,
                        RhsCommentInfo::frozen_only(None),
                    )
                };
            }

            // Find colon position and check for comments
            // Use key_region_end (after `]` for computed, after key for normal)
            // to avoid double-counting comments already inside brackets
            let colon_pos = self.find_colon_after(key_region_end);
            let value_start = prop.value.span().start;

            // A line comment — or a multiline block the author broke after — between
            // the key and `:` keeps the comment after the key and drops `: value` to
            // a continuation line indented one level (prettier relocates it —
            // conformance_prettier_ts_comments.md §Comment relocation), bypassing the assignment
            // layout below; a glued block stays inline via the ordinary path.
            if self.comments_force_own_line_between(key_region_end, colon_pos) {
                let value_doc = {
                    let v = self.build_expression_doc(&prop.value);
                    let v = if self
                        .needs_parens(&prop.value, super::ParenContext::ObjectPropertyValue)
                    {
                        d.concat(&[d.text("("), v, d.text(")")])
                    } else {
                        v
                    };
                    self.prepend_rhs_comments(v, colon_pos + 1, value_start)
                };
                let tail = d.concat(&[d.text(": "), value_doc]);
                return d.concat(&[
                    key_doc,
                    self.build_continuation_indent(key_region_end, colon_pos, tail),
                ]);
            }

            // Comments between key region and colon (e.g., {key /* comment */: value})
            let pre_colon_comments: CommentVec<'_> =
                comments_to_emit_in_range(self.comments, key_region_end, colon_pos).collect();
            // Comments between colon and value (e.g., {key: /* comment */ value})
            let post_colon_comments: CommentVec<'_> =
                comments_to_emit_in_range(self.comments, colon_pos + 1, value_start).collect();

            // Check if value needs parens (e.g., assignment expressions)
            let needs_parens =
                self.needs_parens(&prop.value, super::ParenContext::ObjectPropertyValue);

            // A post-colon comment forces break-after-operator when it's a line
            // comment (extends to end of line), an *indentable* block (its reprint is
            // hard lines, which break the group), or the source put the value on a later
            // line than the comment (an own-line leading comment); a block glued to the
            // value stays inline — single-line (`: /* c */ v`) and preserved multi-line
            // (`: /* line1⏎line2 */ v`) alike, since neither carries a break out.
            // Prettier ref: hasLeadingOwnLineComment → break-after-operator in chooseLayout
            //
            // **on page**, not `post_colon_comments` (which is emit-keyed): hanging the
            // value is a LAYOUT decision, so an owned annotation hangs it exactly as any
            // other own-line comment does — even though this gap emits nothing for it (the
            // value's own node prints it, and the `comments_doc` below is empty).
            let has_own_line_comment_post_colon = self
                .comments_in_source_between(colon_pos + 1, value_start)
                .any(|c| {
                    !c.is_block
                        || self.block_comment_is_indentable(c)
                        || !self.is_same_line(c.span.end, value_start)
                });

            // The `:`→value head: an own-line directive there freezes the whole value.
            let value_frozen = self.value_head_frozen_span(colon_pos + 1, prop.value.span());

            if pre_colon_comments.is_empty()
                && post_colon_comments.is_empty()
                && !has_own_line_comment_post_colon
            {
                if needs_parens {
                    // Build manually with parens
                    let value_doc = d.concat(&[
                        d.text("("),
                        self.build_expression_doc(&prop.value),
                        d.text(")"),
                    ]);
                    d.concat(&[key_doc, d.text(": "), value_doc])
                } else {
                    // No parens needed: use unified assignment layout
                    let is_short_key = self.is_short_property_key(&prop.key, prop.computed);
                    self.build_assignment_layout(
                        key_doc,
                        ":",
                        &prop.value,
                        is_short_key,
                        RhsCommentInfo::frozen_only(value_frozen),
                    )
                }
            } else {
                if has_own_line_comment_post_colon {
                    // Line comment or multiline block comment after colon: BreakAfterOperator
                    // Structure: group([group(key + pre_colon), ":", group(indent([line, rhs]))])
                    let mut lhs_parts: DocBuf = smallvec![key_doc];
                    for comment in &pre_colon_comments {
                        lhs_parts.push(d.text(" "));
                        lhs_parts.push(self.build_comment_doc(comment));
                    }
                    // `concat` short-circuits the no-pre-colon-comment case to `key_doc`.
                    let lhs_doc = d.concat(&lhs_parts);

                    // Build RHS: comments (with proper separators) + value
                    let comments_doc = self
                        .build_value_gap_comments_opt(colon_pos + 1, value_start)
                        .unwrap_or_else(|| d.empty());
                    let mut value_parts: DocBuf = smallvec![comments_doc];
                    if needs_parens {
                        value_parts.push(d.text("("));
                    }
                    value_parts.push(match value_frozen {
                        Some(frozen) => self.build_frozen_expression_doc(&prop.value, frozen),
                        None => self.build_expression_doc(&prop.value),
                    });
                    if needs_parens {
                        value_parts.push(d.text(")"));
                    }
                    let rhs_doc = d.concat(&value_parts);

                    // BreakAfterOperator: group([group(left), ":", group(indent([line, rhs]))])
                    d.group(d.concat(&[
                        d.group(lhs_doc),
                        d.text(":"),
                        hang_after_operator(d, rhs_doc),
                    ]))
                } else {
                    // Inline block comments: use assignment layout so choose_layout
                    // applies (e.g., ternary with binaryish test → BreakAfterOperator).
                    // Pre-colon comments become part of the LHS doc.
                    let lhs_doc = if pre_colon_comments.is_empty() {
                        key_doc
                    } else {
                        let mut lhs_parts: DocBuf = smallvec![key_doc];
                        for comment in &pre_colon_comments {
                            lhs_parts.push(d.text(" "));
                            lhs_parts.push(self.build_comment_doc(comment));
                        }
                        d.concat(&lhs_parts)
                    };

                    // Post-colon inline comments become rhs_comments
                    let rhs_comments = if post_colon_comments.is_empty() {
                        None
                    } else {
                        let mut comment_parts: DocBuf = DocBuf::new();
                        for comment in &post_colon_comments {
                            comment_parts.push(self.build_comment_doc(comment));
                            comment_parts.push(d.text(" "));
                        }
                        Some(d.concat(&comment_parts))
                    };

                    if needs_parens {
                        // Rare: assignment expression in object value needs parens
                        let mut parts: DocBuf = smallvec![lhs_doc, d.text(": ")];
                        if let Some(rc) = rhs_comments {
                            parts.push(rc);
                        }
                        parts.push(d.text("("));
                        parts.push(self.build_expression_doc(&prop.value));
                        parts.push(d.text(")"));
                        d.concat(&parts)
                    } else {
                        let is_short_key = self.is_short_property_key(&prop.key, prop.computed);
                        // `value_frozen` is provably `None` on this arm (an own-line
                        // directive is an own-line comment, which took the arm above);
                        // threaded rather than hardcoded so the two can't drift.
                        self.build_assignment_layout(
                            lhs_doc,
                            ":",
                            &prop.value,
                            is_short_key,
                            RhsCommentInfo {
                                comments: rhs_comments,
                                has_line_comment: false,
                                boundary: None,
                                frozen: value_frozen,
                            },
                        )
                    }
                }
            }
        }
    }

    /// Check if a property key is "short" for layout decisions.
    ///
    /// Short keys don't benefit from breaking after the colon.
    /// Complex expressions (calls, binary, etc.) are never short - they can't
    /// be reduced to a simple width, matching Prettier's `cleanDoc` behavior.
    ///
    /// Prettier ref: `isObjectPropertyWithShortKey` in print/assignment.js:401
    /// Uses `getStringWidth(cleanDoc(keyDoc)) < tabWidth + MIN_OVERLAP_FOR_BREAK`
    fn is_short_property_key(&self, key: &Expression<'_>, computed: bool) -> bool {
        // Prettier: MIN_OVERLAP_FOR_BREAK = 3 (assignment.js:409)
        let threshold = TAB_WIDTH + super::assignment::MIN_OVERLAP_FOR_BREAK;

        let base_width = match key {
            // Prettier: cleanDoc reduces identifier keys to their name string
            Expression::Identifier(id) => self.with_ident_name(id, |s| visual_width(s, TAB_WIDTH)),
            Expression::Literal(lit) => match &lit.value {
                LiteralValue::String(cooked) => {
                    let content = cooked.resolve(lit.span, self.source);
                    // For computed keys, quotes are always preserved: ["x"] prints as ['x']
                    // For non-computed keys, valid identifiers are unquoted: {"x":1} → {x:1}
                    // Escape-bearing keys keep their quotes (see `string_key_unquotes`).
                    if computed || !self.string_key_unquotes(lit, content) {
                        visual_width(content, TAB_WIDTH) + 2 // Include quotes
                    } else {
                        visual_width(content, TAB_WIDTH)
                    }
                }
                LiteralValue::Number(_) => {
                    // Use span to get actual source width
                    (lit.span.end - lit.span.start) as usize
                }
                // Other literals (bool, null, etc.) - rare as keys, not short
                _ => return false,
            },
            // Complex expressions (calls, binary, member, etc.) are never "short".
            // Prettier's cleanDoc can't reduce them to strings, so it returns false.
            _ => return false,
        };

        let total_width = if computed {
            base_width + 2 // Add brackets
        } else {
            base_width
        };

        total_width < threshold
    }

    /// A quoted string key may be unquoted only when its *raw* source (escape
    /// sequences intact) is already a valid identifier. Keys whose raw form
    /// differs from the decoded value carry escapes (`'b'`, `'\a'`,
    /// `'\x66\x69\x73\x6b\x65\x72'`) and keep their quotes so the escapes are
    /// preserved — matching Prettier, which only unquotes when
    /// `rawText.slice(1, -1) === value`. Unquoting from the decoded value would
    /// silently rewrite the source text (data loss).
    pub(in crate::printer) fn string_key_unquotes(&self, lit: &Literal<'_>, content: &str) -> bool {
        if !is_valid_js_identifier(content) {
            return false;
        }
        let raw = lit.span.extract(self.source);
        // Strip the surrounding quotes; compare the raw inner text to the
        // decoded value. Equal ⇒ no escapes ⇒ safe to unquote.
        raw.len() >= 2 && raw[1..raw.len() - 1] == *content
    }

    /// Emit a string-literal key with prettier's `quoteProps: as-needed`: drop the
    /// quotes when the raw text is already a valid identifier (`'type'` → `type`),
    /// else keep them and normalize the quote style. Keeping quotes covers
    /// non-identifier keys (`'kebab-case'`) and escape-bearing keys (`'b'`) whose
    /// escapes must be preserved (see [`Self::string_key_unquotes`]). Shared by
    /// object property keys and import-attribute keys.
    pub(in crate::printer) fn build_string_literal_key_doc(
        &self,
        lit: &Literal<'_>,
        content: &str,
    ) -> DocId {
        let d = self.d();
        if self.string_key_unquotes(lit, content) {
            // Unquoted: the key is a bare identifier equal to the literal's inner
            // source slice (`string_key_unquotes` requires a valid identifier, so
            // there are no escapes) — emit that inner span verbatim, no allocation.
            let inner = Span::new(lit.span.start + 1, lit.span.end - 1);
            debug_assert_eq!(inner.extract(self.source), content);
            d.source_span(inner, self.source)
        } else {
            self.build_string_literal_doc(lit)
        }
    }

    /// Build a Doc for a property key
    ///
    /// String literal keys that are valid identifiers are output without quotes.
    /// Example: `{"key": 1}` → `{key: 1}`, but `{"kebab-case": 1}` keeps quotes.
    pub(in crate::printer) fn build_property_key_doc(&self, key: &Expression<'_>) -> DocId {
        match key {
            Expression::Literal(
                lit @ Literal {
                    value: LiteralValue::String(cooked),
                    ..
                },
            ) => {
                let content = cooked.resolve(lit.span, self.source);
                self.build_string_literal_key_doc(lit, content)
            }
            _ => self.build_expression_doc(key),
        }
    }

    /// Emit a type-member key (`PropertySignature`/`MethodSignature`), returning
    /// `(doc, key_region_end)` where `key_region_end` is the source offset just
    /// past the key — after the `]` for computed keys — used to anchor the search
    /// for following comments/modifiers.
    ///
    /// Drops quotes from an identifier-valid string-literal key for property
    /// signatures (`'plain': T` → `plain: T`) and method signatures (`'foo'(): void`
    /// → `foo(): void` — prettier 3.9 unquotes these too). Computed keys are always
    /// emitted verbatim inside their brackets.
    pub(in crate::printer) fn build_type_member_key_doc(
        &self,
        search_start: u32,
        key: &Expression<'_>,
        computed: bool,
    ) -> (DocId, u32) {
        if computed {
            let key_doc = self.build_expression_doc(key);
            self.build_computed_key_bracket_doc(search_start, key, key_doc)
        } else {
            (self.build_property_key_doc(key), key.span().end)
        }
    }

    /// Find the position of `:` after a position (for finding colon in property)
    /// Skips over comments to avoid matching colons inside them.
    pub(in crate::printer) fn find_colon_after(&self, start: u32) -> u32 {
        find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            self.source.len(),
            b':',
        )
        .map_or(start, |pos| pos as u32)
    }

    /// Parenthesize a computed `[expr]` key when the expression needs it for
    /// clarity — an assignment (`[(x = 0)]`) or, inside a for-header init, an `in`
    /// binary (`for (…[(a in b)]…)`, via `needs_parens`' ambient for-init rule).
    /// Shared by object and class computed property/method keys. (The computed
    /// member-*access* index applies the same rule inline in the chain printer,
    /// which threads `in_for_init` explicitly.)
    pub(in crate::printer) fn build_computed_key_expr_doc(&self, key: &Expression<'_>) -> DocId {
        if self.needs_parens(key, super::ParenContext::ComputedPropertyKey) {
            self.d().parens(self.build_expression_doc(key))
        } else {
            self.build_expression_doc(key)
        }
    }

    /// Build a `[key]` doc with comments preserved inside brackets.
    /// Returns `(doc, key_region_end)` where key_region_end is the position after `]`.
    /// Used by object properties/methods, class methods/properties, destructuring
    /// patterns, and interface/type-literal members (via `build_type_member_key_doc`).
    ///
    /// `[`→key comment placement: a block comment hugs `[` inline (`[/* c */ foo]`)
    /// and the bracket stays flat; a **line** comment can't sit inline before the
    /// key (a `//` runs to EOL and would swallow it), so it forces the bracket to
    /// break — preserved where the author wrote it (on the `[` line via
    /// `delimiter_line_comment_prefix`, or on its own line) with the key dropped to
    /// an indented continuation. Prettier relocates such a comment (out to the
    /// member's leading line, or glued flush to the key) — a divergence
    /// (conformance_prettier_ts_comments.md §Comment relocation, "Object/array/block
    /// open-delimiter trailing"). A computed key never breaks on width alone
    /// (prettier keeps a long key inline), so the flat, no-line-comment path stays
    /// verbatim — only a `[`→key line comment switches to the breaking layout.
    pub(in crate::printer) fn build_computed_key_bracket_doc(
        &self,
        search_start: u32,
        key: &Expression<'_>,
        key_doc: DocId,
    ) -> (DocId, u32) {
        let d = self.d();
        let key_start = key.span().start;
        let key_end = key.span().end;
        let bracket_start = self.find_opening_bracket_after(search_start, key_start);
        let bracket_end = self.find_closing_bracket_after(key_end);

        let bracket_line = self.has_line_comments_between(bracket_start + 1, key_start);
        let after_key_line = self.has_line_comments_between(key_end, bracket_end);

        // Flat path (no line comment in either in-bracket gap): block comments hug
        // inline (`[/* d */ foo]`, `[foo /* c */]`), the key never breaks on width.
        // Byte-identical to the pre-divergence behavior.
        if !bracket_line && !after_key_line {
            let mut parts: DocBuf = smallvec![d.text("[")];
            for comment in comments_to_emit_in_range(self.comments, bracket_start + 1, key_start) {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
            parts.push(key_doc);
            for comment in comments_to_emit_in_range(self.comments, key_end, bracket_end) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.text("]"));
            return (d.concat(&parts), bracket_end + 1);
        }

        // Breaking path: a line comment in either in-bracket gap forces the bracket
        // to break so the `//` can't swallow the key or `]`, preserving each comment
        // in place. `[`→key: a `[`-line comment is pulled onto the `[` line, an
        // own-line one stays on its own line (`build_leading_comments_multiline*`,
        // the shared open-delimiter leading-comment builder, hugging a same-line
        // block to the key). key→`]`: a same-line comment trails the key with a
        // space, an own-line comment keeps its own line. Prettier relocates instead
        // (conformance_prettier_ts_comments.md §Comment relocation).
        // Build the body (key + any key→`]` trailing comments) into a buffer; the shared
        // bracket-break helper owns the `[`→key line-comment prefix and the break shell.
        let mut body_parts: DocBuf = smallvec![key_doc];
        let mut prev = key_end;
        for comment in comments_to_emit_in_range(self.comments, key_end, bracket_end) {
            if self.is_same_line(prev, comment.span.start) {
                body_parts.push(d.text(" "));
            } else {
                body_parts.push(d.hardline());
            }
            body_parts.push(self.build_comment_doc(comment));
            prev = comment.span.end;
        }
        let bracket = self.build_bracket_line_comment_break(
            "[",
            bracket_start,
            key_start,
            d.concat(&body_parts),
        );
        (bracket, bracket_end + 1)
    }

    /// Find the opening `[` bracket between two positions (for computed properties).
    /// Returns the first `[` found outside comments in the range [start, end).
    pub(in crate::printer) fn find_opening_bracket_after(&self, start: u32, end: u32) -> u32 {
        find_char_skipping_comments(self.source.as_bytes(), start as usize, end as usize, b'[')
            .map_or(start, |pos| pos as u32)
    }

    /// Find the closing `]` bracket after a position (for computed properties)
    /// Skips over comments to avoid matching brackets inside them.
    fn find_closing_bracket_after(&self, pos: u32) -> u32 {
        find_char_skipping_comments(
            self.source.as_bytes(),
            pos as usize,
            self.source.len(),
            b']',
        )
        .map_or(pos + 1, |p| p as u32)
    }
}
