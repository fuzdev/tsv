// Literal type printing for TypeScript
//
// Handles:
// - String literal types: `"hello"`
// - Number literal types: `42`
// - BigInt literal types: `100n`
// - Unary expression types: `-1`
// - Template literal types: `\`prefix${T}suffix\``

use super::Printer;
use crate::ast::internal::{TSLiteralType, TSType, TemplateElement, TemplateLiteralType};
use crate::printer::analysis::has_newline_before_position;
use smallvec::{SmallVec, smallvec};
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::visual_width;
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};

/// How a `${…}` interpolation in a template literal type breaks.
///
/// Decided in the first pass at flat-layout positions so the second pass can emit
/// each interpolation with its break already chosen (see `build_template_literal_type_doc`).
enum InterpolationLayout {
    /// A comment that hangs the type ([`Printer::comment_hangs_next`]) **and was authored
    /// on its own line**: the interpolation expands so the comment keeps that line
    /// (`` `a${⏎/* c */⏎T⏎}` ``), matching prettier. A comment authored *trailing* `${`
    /// takes the flush layouts below instead and stays trailing — the authored break is
    /// preserved either way, the same distinction the type-alias `=` gap draws
    /// (`type A = /* c */ B` hugs, `type A =⏎/* c */⏎B` keeps its own line). Takes
    /// precedence over the width-driven layouts: the comment forces the break whatever
    /// the width, and a `//` must not swallow the type.
    Expanded,
    /// A comment that hangs the type but was authored **trailing** `${`: the comment keeps
    /// that line and the type drops below it (`` `a${// c⏎T⏎}` ``). Identical to
    /// [`InterpolationLayout::Expanded`] but for the leading `hardline` — that one newline
    /// *is* the authoring distinction.
    TrailingComment,
    /// Conditional type: wrap in a group; breaks happen at the `?`/`:` operators.
    Conditional,
    /// Exceeds print width at its flat position — always break after `${`.
    Forced,
    /// Short enough at its flat position — try inline first, break if it doesn't fit at the
    /// actual render position. Carries no payload: the flat rendering is a *measurement*
    /// (it decides this variant vs `Forced`, and advances the flat-position cursor), never
    /// output — the inline candidate holds the type's own doc.
    Flex,
}

/// One `${…}` interpolation's docs plus its computed layout.
struct Interpolation {
    type_doc: DocId,
    comments_doc: DocId,
    layout: InterpolationLayout,
}

impl<'a> Printer<'a> {
    /// The `${` opening an interpolation sits immediately after its quasi's raw text
    /// (delimiters excluded), so `raw_span.end` is its start — an anchor that ignores both
    /// an escaped `\${` inside the quasi and a `${` buried in a trailing comment (a raw
    /// `rfind` matched the latter and dropped the comment between `${` and the type).
    fn interp_dollar_brace_end(quasi: &TemplateElement<'_>) -> u32 {
        quasi.raw_span.end + "${".len() as u32
    }

    /// Whether any `${…}` interpolation holds a comment that hangs its type — i.e. whether
    /// the template breaks itself, in *either* the expanded or the flush layout.
    ///
    /// One question, one predicate: the layout decision in
    /// [`Self::build_template_literal_type_doc`] and the type-alias `=` layout (which keeps
    /// the backtick on the `=` line when the template already breaks itself) both ask this,
    /// so they cannot disagree about whether the template breaks. Deliberately *not* keyed
    /// on the expanded-vs-flush choice: the `=` hugs either way, since either layout drops
    /// the type below the backtick.
    pub(in crate::printer) fn template_literal_type_breaks_for_comment(
        &self,
        template: &TemplateLiteralType<'_>,
    ) -> bool {
        template
            .quasis
            .iter()
            .zip(template.types.iter())
            .any(|(quasi, t)| {
                self.comments_force_own_line_between(
                    Self::interp_dollar_brace_end(quasi),
                    t.span().start,
                )
            })
    }

    /// Whether a `${`→type gap's comment run was authored on its own line (a newline before
    /// the first comment) rather than trailing `${`.
    ///
    /// Keyed on the newline *before* the comment — the authoring axis — where
    /// [`Printer::comment_hangs_next`] keys on the newline *after* it. The two are
    /// independent: what follows the comment decides whether the type hangs at all, what
    /// precedes it decides whether the comment keeps its own line. Note
    /// [`Printer::is_own_line_comment`] answers a different question (it treats every line
    /// comment as own-line, which a trailing `${ // c` is not).
    fn interp_comment_authored_own_line(&self, gap_start: u32, gap_end: u32) -> bool {
        comments_to_emit_in_range(self.comments, gap_start, gap_end)
            .next()
            .is_some_and(|c| has_newline_before_position(self.source, c.span.start))
    }

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
                let dollar_brace_end = Self::interp_dollar_brace_end(quasi);
                let type_start = t.span().start;
                // A comment that hangs the type drops the type below it — a `//` can't
                // swallow it, and a multiline block's authored break isn't reflowed. Same
                // gate as the emitter's per-comment rule, so the two can't disagree. Where
                // the comment itself lands is the independent authoring question: own-line
                // authored expands, trailing `${` stays trailing.
                let comment_hangs_type =
                    self.comments_force_own_line_between(dollar_brace_end, type_start);
                let comments_doc =
                    self.build_trailing_comments_hang_next(dollar_brace_end, type_start);
                let type_doc = self.build_type_doc(t);
                let flat_str = self.render_arena_doc_flat(type_doc);

                // Position includes: current pos + "${" (2) + type + "}" (1)
                // (comment width is small enough to ignore for width calculation)
                // `${`/`}` are exact ASCII column counts; the type uses visual width.
                let interp_end = pos + 2 + visual_width(&flat_str, TAB_WIDTH) + 1;
                pos = interp_end;

                // Every hanging comment takes a hang layout, so the width-driven layouts
                // below only ever see a *collapsing* `comments_doc` (a trailing space).
                // That is load-bearing: a hanging `comments_doc` ends in a hardline, and
                // smuggling one into `Flex`'s flat candidate renders it broken with `}`
                // glued (a layout nothing chose), while `Forced` — which opens with its own
                // hardline — emits a blank line after the comment.
                let layout = if comment_hangs_type {
                    if self.interp_comment_authored_own_line(dollar_brace_end, type_start) {
                        InterpolationLayout::Expanded
                    } else {
                        InterpolationLayout::TrailingComment
                    }
                } else if matches!(t, TSType::Conditional(_)) {
                    InterpolationLayout::Conditional
                } else if interp_end > PRINT_WIDTH {
                    InterpolationLayout::Forced
                } else {
                    InterpolationLayout::Flex
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
                    // `comments_doc` already ends in the hardline that hangs the type, so
                    // indenting it together with the type puts the type below the comment.
                    // The two hang layouts differ only in the leading `hardline`, which is
                    // what drops the comment off the `${` line. `}` lands on its own line in
                    // both, as it does for a width-driven break.
                    InterpolationLayout::Expanded => d.concat(&[
                        d.text("${"),
                        d.indent(d.concat(&[d.hardline(), comments_doc, type_doc])),
                        d.hardline(),
                        d.text("}"),
                    ]),
                    InterpolationLayout::TrailingComment => d.concat(&[
                        d.text("${"),
                        d.indent(d.concat(&[comments_doc, type_doc])),
                        d.hardline(),
                        d.text("}"),
                    ]),
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
                    InterpolationLayout::Flex => d.conditional_group(&[
                        // `type_doc`, not the flat string it rendered to: a pre-rendered blob
                        // carries the type's comments out as plain text, so their doc nodes
                        // never render and the print-once ledger cannot see them (a comment
                        // genuinely lost inside such a blob would trip no gate). Both
                        // candidates hold the same subtree and exactly one renders, so each
                        // comment records exactly one emit. The flat string stays what it
                        // always was — a width measurement — and is not output.
                        d.concat(&[d.text("${"), comments_doc, type_doc, d.text("}")]),
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
