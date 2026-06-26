use crate::ast::internal::CssValue;
use crate::number::number_part_len;
use tsv_lang::Span;

/// Parse dimension value: "10px", "1.5em", "50%", or unitless number.
///
/// Only the number/unit split is needed to *classify* the token as a dimension;
/// the unit text is recovered from `span` at print time, so it is not stored.
pub fn parse_dimension<'arena>(s: &str, span: Span) -> Option<CssValue<'arena>> {
    let (number, _unit) = parse_dimension_parts(s)?;
    Some(CssValue::Dimension {
        value: number,
        span,
    })
}

/// Split a dimension string into its numeric value and unit, returning `None`
/// when it doesn't start with a number. Uses the shared CSS number grammar so
/// exponents and trailing dots are handled the same way as the lexer and
/// printer (`1.5e10` → `(15000000000.0, "")`, `1.px` → `(1.0, ".px")`).
/// The unit is a borrowed sub-slice of `s`; the caller discards it (the unit text is
/// recovered from `span` at print time) and keeps only the classification + number.
fn parse_dimension_parts(s: &str) -> Option<(f64, &str)> {
    let num_end = number_part_len(s);
    if num_end == 0 {
        return None;
    }

    let number = s[..num_end].parse::<f64>().ok()?;
    let unit = &s[num_end..];

    Some((number, unit))
}
