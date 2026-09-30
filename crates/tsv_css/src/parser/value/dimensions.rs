use crate::ast::internal::CssValue;
use crate::number::starts_number;
use tsv_lang::Span;

/// Parse dimension value: "10px", "1.5em", "50%", or unitless number.
///
/// Only the classification is decided here — whether `s` opens with a number under the
/// shared CSS number grammar (`starts_number`, the head of `number_part_len`, which the
/// printer's normalization reads for the split; the lexer shares its `exponent_len` /
/// `continues_unit` pieces) — so exponents and trailing dots are handled the same way
/// everywhere (`1.5e10`, `1.px`). The number and unit text are recovered from `span` at
/// print time, so neither is stored.
pub fn parse_dimension<'arena>(s: &str, span: Span) -> Option<CssValue<'arena>> {
    starts_number(s).then_some(CssValue::Dimension { span })
}
