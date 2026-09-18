use crate::ast::internal::CssValue;
use crate::number::number_part_len;
use tsv_lang::Span;

/// Parse dimension value: "10px", "1.5em", "50%", or unitless number.
///
/// Only the classification is decided here — whether `s` opens with a number under the
/// shared CSS number grammar (`number_part_len`, the one the printer's normalization reads;
/// the lexer shares its `exponent_len` / `continues_unit` pieces) — so exponents and trailing
/// dots are handled the same way everywhere (`1.5e10`, `1.px`). The
/// number and unit text are recovered from `span` at print time, so neither is stored.
pub fn parse_dimension<'arena>(s: &str, span: Span) -> Option<CssValue<'arena>> {
    (number_part_len(s) > 0).then_some(CssValue::Dimension { span })
}
