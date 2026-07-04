// Literal type printing for TypeScript
//
// Handles:
// - String literal types: `"hello"`
// - Number literal types: `42`
// - BigInt literal types: `100n`
// - Unary expression types: `-1`
// - Template literal types: `\`prefix${T}suffix\``

use super::Printer;
use crate::ast::internal::{TSLiteralType, TSType, TemplateLiteralType};
use smallvec::{SmallVec, smallvec};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::visual_width;
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};

/// How a `${…}` interpolation in a template literal type breaks.
///
/// Decided in the first pass at flat-layout positions so the second pass can emit
/// each interpolation with its break already chosen (see `build_template_literal_type_doc`).
enum InterpolationLayout {
    /// Conditional type: wrap in a group; breaks happen at the `?`/`:` operators.
    Conditional,
    /// Exceeds print width at its flat position — always break after `${`.
    Forced,
    /// Short enough — try the flat-rendered string first, break if it doesn't fit
    /// at the actual render position.
    Flex(String),
}

/// One `${…}` interpolation's docs plus its computed layout.
struct Interpolation {
    type_doc: DocId,
    comments_doc: DocId,
    layout: InterpolationLayout,
}

impl<'a> Printer<'a> {
    /// Build a Doc for a literal type
    pub(super) fn build_literal_type_doc(&self, lit: &TSLiteralType<'_>) -> DocId {
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
                let arg = self.build_expression_doc(unary.argument);
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
    pub(super) fn build_template_literal_type_doc(
        &self,
        template: &TemplateLiteralType<'_>,
    ) -> DocId {
        let d = self.d();

        // First pass: render each type flat and decide its break at its flat position.
        let mut interps: SmallVec<[Interpolation; 4]> =
            SmallVec::with_capacity(template.types.len());
        let mut pos: usize = 1; // Start after backtick

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Visual columns, not bytes: a CJK char is 3 bytes but 2 columns, so a
            // byte sum would break a template whose rendered width is well under print width.
            pos += visual_width(quasi.raw(self.source), TAB_WIDTH);
            if i < template.types.len() {
                let t = &template.types[i];
                // Comments between `${` and the type
                // Find the `${` by searching backward from the type start
                let search_start = quasi.span.start;
                let dollar_brace_end = self.source[search_start as usize..t.span().start as usize]
                    .rfind("${")
                    .map_or(quasi.span.end, |p| {
                        search_start + p as u32 + "${".len() as u32
                    });
                let type_start = t.span().start;
                // A line comment breaks so it can't swallow the interpolation type.
                let comments_doc =
                    self.build_trailing_comments_break_for_line(dollar_brace_end, type_start);
                let type_doc = self.build_type_doc(t);
                let flat_str = self.render_arena_doc_flat(type_doc);

                // Position includes: current pos + "${" (2) + type + "}" (1)
                // (comment width is small enough to ignore for width calculation)
                // `${`/`}` are exact ASCII column counts; the type uses visual width.
                let interp_end = pos + 2 + visual_width(&flat_str, TAB_WIDTH) + 1;
                pos = interp_end;

                let layout = if matches!(t, TSType::Conditional(_)) {
                    InterpolationLayout::Conditional
                } else if interp_end > PRINT_WIDTH {
                    InterpolationLayout::Forced
                } else {
                    InterpolationLayout::Flex(flat_str)
                };
                interps.push(Interpolation {
                    type_doc,
                    comments_doc,
                    layout,
                });
            }
        }

        // Second pass: build doc with breaking decisions already made.
        // indent() positions content relative to the template's current indent level
        // (e.g. after an `=` break in a type alias): content gets +1, the closing `}`
        // stays at the current level.
        let mut parts: DocBuf = smallvec![d.text("`")];
        let mut interp_iter = interps.into_iter();

        for quasi in template.quasis {
            // Template raw text is a verbatim source slice (`raw_span`) — emit it
            // without allocating.
            parts.push(d.source_span(quasi.raw_span, self.source));
            if let Some(Interpolation {
                type_doc,
                comments_doc,
                layout,
            }) = interp_iter.next()
            {
                let interp_doc = match layout {
                    // Conditional type's own formatting handles branch indentation, so no extra indent.
                    InterpolationLayout::Conditional => {
                        d.concat(&[d.text("${"), comments_doc, d.group(type_doc), d.text("}")])
                    }
                    InterpolationLayout::Forced => d.concat(&[
                        d.text("${"),
                        comments_doc,
                        d.indent(d.concat(&[d.hardline(), type_doc])),
                        d.hardline(),
                        d.text("}"),
                    ]),
                    InterpolationLayout::Flex(flat_str) => d.conditional_group(&[
                        d.concat(&[
                            d.text("${"),
                            comments_doc,
                            d.text_pooled(&flat_str),
                            d.text("}"),
                        ]),
                        d.concat(&[
                            d.text("${"),
                            comments_doc,
                            d.indent_line(type_doc),
                            d.line(),
                            d.text("}"),
                        ]),
                    ]),
                };
                parts.push(interp_doc);
            }
        }
        parts.push(d.text("`"));
        d.concat(&parts)
    }
}
