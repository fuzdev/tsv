// Array expression printing for TypeScript
//
// Handles printing of array expressions with:
// - Width-based wrapping
// - Fill mode for number-only arrays
// - Forced expansion for multiline content

use super::{Printer, has_multiline_content};
use crate::ast::internal::{self, Expression, LiteralValue};
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Print an array expression: `[1, 2, 3]`
    pub(super) fn print_array_expression(&mut self, arr: &internal::ArrayExpression) {
        if arr.elements.is_empty() {
            self.write("[]");
            return;
        }

        // Build doc for width-based wrapping
        let doc = self.build_array_doc_with_wrapping(arr);
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1; // +1 for trailing punctuation
        let output = doc::print_doc_at_column(&doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for an array with proper wrapping behavior
    pub(super) fn build_array_doc_with_wrapping(&self, arr: &internal::ArrayExpression) -> Doc {
        if arr.elements.is_empty() {
            return doc::text("[]");
        }

        // Check if any element has multiline content (e.g., line continuation strings)
        // Prettier expands arrays containing multiline strings (recursively)
        let has_multiline = arr
            .elements
            .iter()
            .flatten()
            .any(|elem| has_multiline_content(elem, self.source));

        // Check if this is a "numbers-only" array (use fill) vs other (one-per-line)
        let is_numbers_only = self.is_numbers_only_array(arr);

        if has_multiline {
            // Force expansion with hardlines for multiline content
            self.build_array_group_doc_forced(arr)
        } else if is_numbers_only {
            // Use fill for greedy packing of numbers
            self.build_array_fill_doc(arr)
        } else {
            // Use group with one-per-line for other content
            self.build_array_group_doc(arr)
        }
    }

    /// Check if array contains only numeric literals (for fill behavior)
    fn is_numbers_only_array(&self, arr: &internal::ArrayExpression) -> bool {
        arr.elements.iter().all(|elem| match elem {
            Some(Expression::Literal(lit)) => {
                matches!(lit.value, LiteralValue::Number(_))
            }
            Some(Expression::UnaryExpression(unary)) => {
                // -1, +1 are also numeric
                matches!(
                    unary.operator,
                    internal::UnaryOperator::Minus | internal::UnaryOperator::Plus
                ) && matches!(
                    unary.argument.as_ref(),
                    Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
                )
            }
            _ => false,
        })
    }

    /// Build fill doc for numbers-only arrays (greedy packing)
    fn build_array_fill_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            if i < arr.elements.len() - 1 {
                // Separator: comma + line (becomes space in flat mode, newline in break)
                parts.push(doc::concat(vec![doc::text(","), doc::line()]));
            }
        }

        // In multi-declarator context, apply extra indentation
        let inner = doc::concat(vec![
            doc::softline(),
            doc::fill(parts),
            doc::if_break(doc::text(","), doc::text("")),
        ]);
        let (indented_content, closing_line) = if self.declaration_indent_depth > 0 {
            (
                doc::indent(doc::indent(inner)),
                doc::indent(doc::softline()),
            )
        } else {
            (doc::indent(inner), doc::softline())
        };

        doc::group(doc::concat(vec![
            doc::text("["),
            indented_content,
            closing_line,
            doc::text("]"),
        ]))
    }

    /// Build group doc for non-numeric arrays (one per line when broken)
    fn build_array_group_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            if i < arr.elements.len() - 1 {
                parts.push(doc::text(","));
                parts.push(doc::line());
            }
        }

        // In multi-declarator context, apply extra indentation
        let inner = doc::concat(vec![
            doc::softline(),
            doc::concat(parts),
            doc::if_break(doc::text(","), doc::text("")),
        ]);
        let (indented_content, closing_line) = if self.declaration_indent_depth > 0 {
            (
                doc::indent(doc::indent(inner)),
                doc::indent(doc::softline()),
            )
        } else {
            (doc::indent(inner), doc::softline())
        };

        doc::group(doc::concat(vec![
            doc::text("["),
            indented_content,
            closing_line,
            doc::text("]"),
        ]))
    }

    /// Build group doc for arrays with multiline content (forced expansion with hardlines)
    fn build_array_group_doc_forced(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            if i < arr.elements.len() - 1 {
                parts.push(doc::text(","));
                parts.push(doc::hardline());
            }
        }

        // In multi-declarator context, apply extra indentation
        let inner = doc::concat(vec![doc::hardline(), doc::concat(parts), doc::text(",")]);
        let (indented_content, closing_line) = if self.declaration_indent_depth > 0 {
            (
                doc::indent(doc::indent(inner)),
                doc::indent(doc::hardline()),
            )
        } else {
            (doc::indent(inner), doc::hardline())
        };

        doc::concat(vec![
            doc::text("["),
            indented_content,
            closing_line,
            doc::text("]"),
        ])
    }

    /// Build a Doc for an array expression (for nested contexts)
    ///
    /// Delegates to `build_array_doc_with_wrapping` to ensure multiline content
    /// triggers proper expansion even in nested contexts.
    pub(super) fn build_array_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        // Use the same wrapping logic as top-level arrays to handle multiline content
        self.build_array_doc_with_wrapping(arr)
    }
}
