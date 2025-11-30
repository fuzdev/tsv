// Call and member expression printing for TypeScript
//
// Handles printing of:
// - Call expressions: `foo()`, `obj.method(arg1, arg2)`
// - Member expressions: `obj.prop`, `arr[0]`
// - Method chains: `arr.filter().map()`
// - Conditional expressions: `a ? b : c`

use super::{Printer, has_multiline_content};
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};

/// Check if a chain expression contains any call expressions
fn chain_has_calls(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::CallExpression(_) => true,
        internal::Expression::MemberExpression(member) => chain_has_calls(&member.object),
        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Print a call expression: `foo()`, `obj.method(arg1, arg2)`
    ///
    /// For method chains like `arr.filter().map()`, wraps with leading `.`:
    /// ```javascript
    /// arr
    ///     .filter(...)
    ///     .map(...)
    /// ```
    ///
    /// For standalone calls, wraps args when they exceed print_width:
    /// ```javascript
    /// fn(
    ///     arg1,
    ///     arg2,
    /// )
    /// ```
    pub(super) fn print_call_expression(&mut self, call: &internal::CallExpression) {
        // Check if this is part of a chain (callee is member or call expression)
        let is_chain = matches!(
            &*call.callee,
            internal::Expression::MemberExpression(_) | internal::Expression::CallExpression(_)
        );

        let doc = if is_chain {
            // Use chain wrapping - collect all segments
            self.build_chain_doc_with_wrapping(&internal::Expression::CallExpression(call.clone()))
        } else {
            // Simple call - just wrap args
            self.build_call_doc_with_wrapping(call)
        };

        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1;
        let output = doc::print_doc_at_column(&doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for a call expression with argument wrapping (not chain-aware)
    pub(super) fn build_call_doc_with_wrapping(&self, call: &internal::CallExpression) -> Doc {
        let callee = self.build_expression_doc(&call.callee);

        // Handle optional chaining
        let callee = if call.optional {
            doc::concat(vec![callee, doc::text("?.")])
        } else {
            callee
        };

        // Empty args: just `fn()`
        if call.arguments.is_empty() {
            return doc::concat(vec![callee, doc::text("()")]);
        }

        // Check if any argument has multiline content (e.g., line continuation strings)
        // Prettier expands calls containing multiline strings (recursively)
        let has_multiline = call
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            let mut arg_parts = Vec::new();
            for (i, arg) in call.arguments.iter().enumerate() {
                if i > 0 {
                    arg_parts.push(doc::text(","));
                    arg_parts.push(doc::hardline());
                }
                arg_parts.push(self.build_expression_doc(arg));
            }

            // Always expanded with trailing comma
            return doc::concat(vec![
                callee,
                doc::text("("),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    doc::concat(arg_parts),
                    doc::text(","),
                ])),
                doc::hardline(),
                doc::text(")"),
            ]);
        }

        // Build args with line separators (one per line when broken)
        let mut arg_parts = Vec::new();
        for (i, arg) in call.arguments.iter().enumerate() {
            if i > 0 {
                arg_parts.push(doc::text(","));
                arg_parts.push(doc::line());
            }
            arg_parts.push(self.build_expression_doc(arg));
        }

        // Wrap in group with parens
        doc::group(doc::concat(vec![
            callee,
            doc::text("("),
            doc::indent(doc::concat(vec![
                doc::softline(),
                doc::concat(arg_parts),
                doc::if_break(doc::text(","), doc::text("")),
            ])),
            doc::softline(),
            doc::text(")"),
        ]))
    }

    /// Build a Doc for a chain (method chain or member chain) with wrapping
    fn build_chain_doc_with_wrapping(&self, expr: &internal::Expression) -> Doc {
        let segments = self.collect_chain_segments(expr);

        if segments.len() <= 1 {
            return segments.into_iter().next().unwrap_or_else(|| doc::text(""));
        }

        // Build chain with optional breaks between segments
        let mut parts = Vec::new();
        for (i, segment) in segments.into_iter().enumerate() {
            if i == 0 {
                parts.push(segment);
            } else {
                // Each subsequent segment can break with indent
                parts.push(doc::indent(doc::concat(vec![doc::softline(), segment])));
            }
        }

        doc::group(doc::concat(parts))
    }

    /// Print a new expression: `new Date()`, `new Map()`
    ///
    /// For constructor calls, wraps args when they exceed print_width.
    pub(super) fn print_new_expression(&mut self, new_expr: &internal::NewExpression) {
        let doc = self.build_new_doc_with_wrapping(new_expr);
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1;
        let output = doc::print_doc_at_column(&doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for a new expression with argument wrapping
    pub(super) fn build_new_doc_with_wrapping(&self, new_expr: &internal::NewExpression) -> Doc {
        let callee = self.build_expression_doc(&new_expr.callee);

        // Empty args: just `new Foo()`
        if new_expr.arguments.is_empty() {
            return doc::concat(vec![doc::text("new "), callee, doc::text("()")]);
        }

        // Check if any argument has multiline content
        let has_multiline = new_expr
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            let mut arg_parts = Vec::new();
            for (i, arg) in new_expr.arguments.iter().enumerate() {
                if i > 0 {
                    arg_parts.push(doc::text(","));
                    arg_parts.push(doc::hardline());
                }
                arg_parts.push(self.build_expression_doc(arg));
            }

            return doc::concat(vec![
                doc::text("new "),
                callee,
                doc::text("("),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    doc::concat(arg_parts),
                    doc::text(","),
                ])),
                doc::hardline(),
                doc::text(")"),
            ]);
        }

        // Build args with line separators (one per line when broken)
        let mut arg_parts = Vec::new();
        for (i, arg) in new_expr.arguments.iter().enumerate() {
            if i > 0 {
                arg_parts.push(doc::text(","));
                arg_parts.push(doc::line());
            }
            arg_parts.push(self.build_expression_doc(arg));
        }

        // Wrap in group with parens
        doc::group(doc::concat(vec![
            doc::text("new "),
            callee,
            doc::text("("),
            doc::indent(doc::concat(vec![
                doc::softline(),
                doc::concat(arg_parts),
                doc::if_break(doc::text(","), doc::text("")),
            ])),
            doc::softline(),
            doc::text(")"),
        ]))
    }

    /// Build a Doc for a new expression (for nested contexts)
    pub(super) fn build_new_doc(&self, new_expr: &internal::NewExpression) -> Doc {
        self.build_new_doc_with_wrapping(new_expr)
    }

    /// Print a member expression: `obj.prop`, `arr[0]`
    ///
    /// For property-only chains, keeps inline (relies on assignment-level wrapping).
    /// For method chains (containing calls), wraps with leading `.`.
    pub(super) fn print_member_expression(&mut self, member: &internal::MemberExpression) {
        // Check if this chain contains any calls (method chain vs property chain)
        let has_calls = chain_has_calls(&internal::Expression::MemberExpression(member.clone()));

        if has_calls {
            // Method chain - use chain wrapping
            let doc = self.build_chain_doc_with_wrapping(&internal::Expression::MemberExpression(
                member.clone(),
            ));
            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset + 1;
            let output = doc::print_doc_at_column(&doc, &self.config, current_col);
            self.write(&output);
        } else {
            // Property chain - keep inline, print directly
            self.print_expression(&member.object);

            if member.computed {
                if member.optional {
                    self.write("?.[");
                } else {
                    self.write("[");
                }
                self.print_expression(&member.property);
                self.write("]");
            } else {
                if member.optional {
                    self.write("?.");
                } else {
                    self.write(".");
                }
                self.print_expression(&member.property);
            }
        }
    }

    /// Collect chain segments from a member/call expression chain
    ///
    /// Flattens `a.b.c().d` into segments: [`a`, `.b`, `.c()`, `.d`]
    fn collect_chain_segments(&self, expr: &internal::Expression) -> Vec<Doc> {
        let mut segments = Vec::new();
        self.collect_chain_segments_recursive(expr, &mut segments);
        segments
    }

    fn collect_chain_segments_recursive(
        &self,
        expr: &internal::Expression,
        segments: &mut Vec<Doc>,
    ) {
        match expr {
            internal::Expression::MemberExpression(member) => {
                // Recurse into object first
                self.collect_chain_segments_recursive(&member.object, segments);

                // Build this segment: `.prop` or `[expr]`
                let segment = if member.computed {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?.["), prop, doc::text("]")])
                    } else {
                        doc::concat(vec![doc::text("["), prop, doc::text("]")])
                    }
                } else {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?."), prop])
                    } else {
                        doc::concat(vec![doc::text("."), prop])
                    }
                };
                segments.push(segment);
            }
            internal::Expression::CallExpression(call) => {
                // Recurse into callee first
                self.collect_chain_segments_recursive(&call.callee, segments);

                // Build this segment: the call arguments `(arg1, arg2)` or `?.(arg1, arg2)`
                // Use wrapping for the args
                let open_paren = if call.optional { "?.(" } else { "(" };

                let args_doc = if call.arguments.is_empty() {
                    doc::text(if call.optional { "?.()" } else { "()" })
                } else {
                    let mut arg_parts = Vec::new();
                    for (i, arg) in call.arguments.iter().enumerate() {
                        if i > 0 {
                            arg_parts.push(doc::text(","));
                            arg_parts.push(doc::line());
                        }
                        arg_parts.push(self.build_expression_doc(arg));
                    }
                    doc::group(doc::concat(vec![
                        doc::text(open_paren),
                        doc::indent(doc::concat(vec![
                            doc::softline(),
                            doc::concat(arg_parts),
                            doc::if_break(doc::text(","), doc::text("")),
                        ])),
                        doc::softline(),
                        doc::text(")"),
                    ]))
                };

                // Append args to the last segment (if any) or create new segment
                if let Some(last) = segments.pop() {
                    segments.push(doc::concat(vec![last, args_doc]));
                } else {
                    segments.push(args_doc);
                }
            }
            // Base case: identifiers, literals, etc.
            _ => {
                segments.push(self.build_expression_doc(expr));
            }
        }
    }

    /// Print a conditional (ternary) expression: `a ? b : c`
    ///
    /// When the line exceeds print_width, wraps to:
    /// ```javascript
    /// longCondition
    ///     ? consequent
    ///     : alternate
    /// ```
    ///
    /// Also wraps nested conditionals in the consequent position with parentheses
    /// for readability: `a ? (b ? c : d) : e`.
    pub(super) fn print_conditional_expression(&mut self, cond: &internal::ConditionalExpression) {
        let doc = self.build_conditional_doc_with_wrapping(cond);
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1;
        let output = doc::print_doc_at_column(&doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for a conditional expression with wrapping support
    pub(super) fn build_conditional_doc_with_wrapping(
        &self,
        cond: &internal::ConditionalExpression,
    ) -> Doc {
        let test = self.build_expression_doc(&cond.test);
        let consequent = self.build_expression_doc(&cond.consequent);
        let alternate = self.build_expression_doc(&cond.alternate);

        // Wrap nested conditional in consequent with parentheses
        let consequent = if matches!(
            &*cond.consequent,
            internal::Expression::ConditionalExpression(_)
        ) {
            doc::concat(vec![doc::text("("), consequent, doc::text(")")])
        } else {
            consequent
        };

        // When flat: `test ? consequent : alternate`
        // When broken: `test\n\t? consequent\n\t: alternate`
        doc::group(doc::concat(vec![
            test,
            doc::indent(doc::concat(vec![
                doc::line(),
                doc::text("? "),
                consequent,
                doc::line(),
                doc::text(": "),
                alternate,
            ])),
        ]))
    }

    /// Build a Doc for a call expression (for nested contexts)
    ///
    /// Delegates to `build_call_doc_with_wrapping` to ensure multiline content
    /// triggers proper expansion even in nested contexts.
    pub(super) fn build_call_doc(&self, call: &internal::CallExpression) -> Doc {
        self.build_call_doc_with_wrapping(call)
    }

    /// Build a Doc for a member expression with optional breaking at dots
    ///
    /// For long property chains, uses greedy line packing - fits as many segments
    /// as possible on each line before breaking:
    /// ```javascript
    /// const long =
    ///     obj.prop1.prop2.prop3.prop4.prop5.prop6.prop7.prop8.prop9.prop10.prop11.prop12.prop13.prop14
    ///         .prop15.prop16;
    /// ```
    pub(super) fn build_member_doc(&self, member: &internal::MemberExpression) -> Doc {
        // Collect all segments of the chain (root + each member access)
        let mut segments: Vec<Doc> = Vec::new();
        self.collect_member_segments(
            &internal::Expression::MemberExpression(member.clone()),
            &mut segments,
        );

        if segments.len() <= 1 {
            return segments.into_iter().next().unwrap_or_else(|| doc::text(""));
        }

        // Build fill parts: [segment, softline, segment, softline, ...]
        // Fill uses greedy packing - fits as many on each line as possible
        let mut fill_parts = Vec::new();
        for (i, segment) in segments.into_iter().enumerate() {
            if i > 0 {
                // Separator before each segment (except first)
                // softline: nothing in flat mode, newline+indent in break mode
                fill_parts.push(doc::softline());
            }
            fill_parts.push(segment);
        }

        // Wrap in group with indent so breaks get proper indentation
        doc::group(doc::indent(doc::fill(fill_parts)))
    }

    /// Collect segments from a member expression chain
    ///
    /// Flattens `a.b.c[d]` into segments: [`a`, `.b`, `.c`, `[d]`]
    fn collect_member_segments(&self, expr: &internal::Expression, segments: &mut Vec<Doc>) {
        match expr {
            internal::Expression::MemberExpression(member) => {
                // Recurse into object first
                self.collect_member_segments(&member.object, segments);

                // Build this segment
                let segment = if member.computed {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?.["), prop, doc::text("]")])
                    } else {
                        doc::concat(vec![doc::text("["), prop, doc::text("]")])
                    }
                } else {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?."), prop])
                    } else {
                        doc::concat(vec![doc::text("."), prop])
                    }
                };
                segments.push(segment);
            }
            // Base case: root of the chain
            _ => {
                segments.push(self.build_expression_doc(expr));
            }
        }
    }
}
