// Literal type printing for TypeScript
//
// Handles:
// - String literal types: `"hello"`
// - Number literal types: `42`
// - BigInt literal types: `100n`
// - Unary expression types: `-1`
// - Template literal types: `\`prefix${T}suffix\``

use super::{CommentSpacing, Printer};
use crate::ast::internal::{TSLiteralType, TSType, TemplateLiteralType};
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a literal type
    pub(super) fn build_literal_type_doc(&self, lit: &TSLiteralType) -> DocId {
        let d = self.d();
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.build_template_literal_type_doc(template)
            }
            TSLiteralType::String(literal) => self.build_literal_doc(literal),
            TSLiteralType::Number(literal) => self.build_literal_doc(literal),
            TSLiteralType::BigInt(literal) => self.build_literal_doc(literal),
            TSLiteralType::UnaryExpression(unary) => {
                // For negative number types like `-1`
                let op = d.text(unary.operator.as_str());
                let arg = self.build_expression_doc(&unary.argument);
                d.concat(&[op, arg])
            }
        }
    }

    /// Build a Doc for a template literal type
    ///
    /// Divergence: When the type exceeds print width, we break after `${` with `}` on its own
    /// line. Prettier keeps template literal types inline regardless of length. For conditional
    /// types, we break at `?/:` operators instead (the conditional's natural break points).
    ///
    /// Breaking decision is based on flat-layout positions: we pre-compute which interpolations
    /// would exceed print width in flat mode, then break all of those. This ensures consistent
    /// formatting - types that would exceed at their original positions break, even if earlier
    /// breaks would have given them more room.
    pub(super) fn build_template_literal_type_doc(&self, template: &TemplateLiteralType) -> DocId {
        let d = self.d();
        let print_width = tsv_lang::PRINT_WIDTH;

        // First pass: analyze types and determine which exceed print width at their flat positions
        // Store as (doc, comments_doc, flat_str, is_conditional, exceeds_width)
        let mut type_data: Vec<(DocId, DocId, String, bool, bool)> =
            Vec::with_capacity(template.types.len());
        let mut pos: usize = 1; // Start after backtick

        for (i, quasi) in template.quasis.iter().enumerate() {
            pos += quasi.raw.len();
            if i < template.types.len() {
                let t = &template.types[i];
                let is_conditional = matches!(t, TSType::Conditional(_));
                // Comments between `${` and the type
                // Find the `${` by searching backward from the type start
                let search_start = quasi.span.start;
                let dollar_brace_end = self.source[search_start as usize..t.span().start as usize]
                    .rfind("${")
                    .map_or(quasi.span.end, |p| search_start + p as u32 + 2);
                let type_start = t.span().start;
                let comments_doc = self.build_comments_between(
                    dollar_brace_end,
                    type_start,
                    CommentSpacing::Trailing,
                );
                let type_doc = self.build_type_doc(t);
                let flat_str = self.render_arena_doc_flat(type_doc);

                // Position includes: current pos + "${" (2) + type + "}" (1)
                // (comment width is small enough to ignore for width calculation)
                let interp_end = pos + 2 + flat_str.len() + 1;
                let exceeds_width = interp_end > print_width;

                type_data.push((
                    type_doc,
                    comments_doc,
                    flat_str,
                    is_conditional,
                    exceeds_width,
                ));
                pos = interp_end;
            }
        }

        // Second pass: build doc with breaking decisions already made
        let mut parts = vec![d.text("`")];
        let mut type_iter = type_data.into_iter();

        for quasi in &template.quasis {
            parts.push(d.text_owned(quasi.raw.clone()));
            if let Some((type_doc, comments_doc, flat_str, is_conditional, exceeds_width)) =
                type_iter.next()
            {
                // Use relative indent() for positioning within the current context.
                // The template is already at some indent level (e.g., after = break in type alias).
                // Content gets +1 indent, closing stays at current level.
                let interp_doc = if is_conditional {
                    // Conditional types: wrap in group - breaks happen at ?/: operators
                    // Don't add extra indent - conditional type's own formatting handles branch indentation
                    d.concat(&[d.text("${"), comments_doc, d.group(type_doc), d.text("}")])
                } else if exceeds_width {
                    // Exceeds print width at flat position - always break
                    d.concat(&[
                        d.text("${"),
                        comments_doc,
                        d.indent(d.concat(&[d.hardline(), type_doc])),
                        d.hardline(),
                        d.text("}"),
                    ])
                } else {
                    // Short enough - try flat first, break if doesn't fit at actual position
                    d.conditional_group(&[
                        d.concat(&[
                            d.text("${"),
                            comments_doc,
                            d.text_owned(flat_str),
                            d.text("}"),
                        ]),
                        d.concat(&[
                            d.text("${"),
                            comments_doc,
                            d.indent_line(type_doc),
                            d.line(),
                            d.text("}"),
                        ]),
                    ])
                };
                parts.push(interp_doc);
            }
        }
        parts.push(d.text("`"));
        d.concat(&parts)
    }
}
