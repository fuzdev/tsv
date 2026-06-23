// Number normalization: canonicalizing numeric value text to prettier's form.

/// Normalize a dimension value from raw source string
///
/// This function matches prettier's exact behavior:
/// - Preserves leading zeros: `01.5px` → `01.5px`
/// - Preserves signs: `+10.0px` → `+10px`, `-0.0px` → `-0px`
/// - Removes trailing zeros: `1.50px` → `1.5px`, `100.0px` → `100px`
/// - Adds leading zero: `.5px` → `0.5px`
///
/// # Arguments
/// * `raw` - The raw dimension string from source (e.g., "01.5px", "+10.0em")
///
/// # Returns
/// Normalized dimension string matching prettier's output
pub(crate) fn normalize_dimension_from_source(raw: &str) -> String {
    let (num_part, unit_part) = split_number_and_unit(raw);

    // Not a number we recognize (e.g. a bare identifier) — leave untouched.
    if num_part.is_empty() {
        return raw.to_string();
    }

    let normalized_num = normalize_css_number(num_part);
    format!("{normalized_num}{unit_part}")
}

/// Split a dimension into its numeric part and trailing unit, e.g.
/// `1.5px` → (`1.5`, `px`), `1.png` → (`1`, `.png`).
fn split_number_and_unit(raw: &str) -> (&str, &str) {
    raw.split_at(crate::number::number_part_len(raw))
}

/// Normalize a CSS number to match prettier's `printNumber` / `printCssNumber`.
///
/// Mantissa: add a leading zero (`.5` → `0.5`), trim trailing fraction zeros
/// and a trailing dot (`1.50` → `1.5`, `1.` → `1`), preserve sign and leading
/// integer zeros. Exponent: lowercase `e`, drop a `+` sign, strip leading
/// zeros (`e+0010` → `e10`), and drop a zero exponent entirely (`5e0` → `5`).
pub(crate) fn normalize_css_number(num: &str) -> String {
    let (mantissa, exponent) = match num.find(['e', 'E']) {
        Some(idx) => (&num[..idx], &num[idx + 1..]),
        None => (num, ""),
    };

    let normalized_mantissa = normalize_decimal_preserving_prefix(mantissa);

    if exponent.is_empty() {
        return normalized_mantissa;
    }

    let (exp_sign, exp_digits) = if let Some(rest) = exponent.strip_prefix('-') {
        ("-", rest)
    } else if let Some(rest) = exponent.strip_prefix('+') {
        ("", rest)
    } else {
        ("", exponent)
    };

    let trimmed_digits = exp_digits.trim_start_matches('0');
    if trimmed_digits.is_empty() {
        // Exponent is zero (`5e0`, `5e-00`) — drop it entirely.
        return normalized_mantissa;
    }

    format!("{normalized_mantissa}e{exp_sign}{trimmed_digits}")
}

/// Known CSS units (lowercase), used to gate number normalization in raw
/// prelude text — only a number with a known unit (or no unit) is normalized,
/// matching prettier's `adjustNumbers` (which checks `css-units-list`).
static CSS_UNITS: phf::Set<&'static str> = phf::phf_set! {
    // Absolute length
    "px", "cm", "mm", "in", "pt", "pc", "q",
    // Font-relative length
    "em", "rem", "ex", "rex", "ch", "rch", "cap", "rcap", "ic", "ric", "lh", "rlh",
    // Viewport-relative length
    "vw", "vh", "vi", "vb", "vmin", "vmax",
    "svw", "svh", "svi", "svb", "svmin", "svmax",
    "lvw", "lvh", "lvi", "lvb", "lvmin", "lvmax",
    "dvw", "dvh", "dvi", "dvb", "dvmin", "dvmax",
    // Container-relative length
    "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax",
    // Angle
    "deg", "grad", "rad", "turn",
    // Time
    "s", "ms",
    // Frequency
    "hz", "khz",
    // Resolution
    "dpi", "dpcm", "dppx", "x",
    // Flex / grid
    "fr",
};

pub(crate) fn is_known_css_unit(unit: &str) -> bool {
    // Fast path: units arrive lowercase, so probe directly and only allocate a
    // lowercased copy when the input actually has uppercase ASCII.
    CSS_UNITS.contains(unit)
        || (unit.bytes().any(|b| b.is_ascii_uppercase())
            && CSS_UNITS.contains(unit.to_ascii_lowercase().as_str()))
}

/// Normalize decimal number while preserving sign and leading zeros
///
/// Examples:
/// - `01.50` → `01.5` (preserve leading zero, trim trailing)
/// - `+10.0` → `+10` (preserve sign, trim trailing)
/// - `-0.0` → `-0` (preserve negative zero)
/// - `.5` → `0.5` (add leading zero)
fn normalize_decimal_preserving_prefix(num: &str) -> String {
    // Extract sign if present
    let (sign, rest) = if let Some(stripped) = num.strip_prefix('-') {
        ("-", stripped)
    } else if let Some(stripped) = num.strip_prefix('+') {
        ("+", stripped)
    } else {
        ("", num)
    };

    // Add leading zero if starts with decimal point
    let with_leading = if rest.starts_with('.') {
        format!("0{rest}")
    } else {
        rest.to_string()
    };

    // Remove trailing zeros after decimal point
    let trimmed = if with_leading.contains('.') {
        let mut s = with_leading;
        // Remove trailing zeros
        while s.ends_with('0') && s.contains('.') {
            s.pop();
        }
        // If we removed all digits after decimal, remove the decimal point too
        if s.ends_with('.') {
            s.pop();
        }
        s
    } else {
        with_leading
    };

    format!("{sign}{trimmed}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_css_number_table() {
        // Mantissa: leading-zero insertion, trailing-zero/dot trimming, with
        // leading integer zeros and the negative-zero sign preserved.
        assert_eq!(normalize_css_number(".5"), "0.5");
        assert_eq!(normalize_css_number("5."), "5");
        assert_eq!(normalize_css_number("1.50"), "1.5");
        assert_eq!(normalize_css_number("00.500"), "00.5");
        assert_eq!(normalize_css_number("-0.0"), "-0");
        // Exponent: lowercase `e`, drop `+`, strip leading zeros, drop a zero exponent.
        assert_eq!(normalize_css_number("5e0"), "5");
        assert_eq!(normalize_css_number("1e+0010"), "1e10");
        assert_eq!(normalize_css_number("1.5E-3"), "1.5e-3");
        // A bare trailing `e` (no exponent digits) drops to the mantissa.
        assert_eq!(normalize_css_number("1e"), "1");
    }
}
