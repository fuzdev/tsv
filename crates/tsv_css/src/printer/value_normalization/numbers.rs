// Number normalization: canonicalizing numeric value text to prettier's form.

use std::borrow::Cow;

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
/// Normalized dimension matching prettier's output. A `Cow::Borrowed` means the
/// dimension is already canonical — neither the number nor the unit was rewritten,
/// so the returned slice **is** `raw`; the caller (`build_dimension_doc`) maps that
/// to an allocation-free `source_span`, mirroring the TS literal path.
pub(crate) fn normalize_dimension_from_source(raw: &str) -> Cow<'_, str> {
    let (num_part, unit_part) = split_number_and_unit(raw);

    // Not a number we recognize (e.g. a bare identifier) — leave untouched.
    if num_part.is_empty() {
        return Cow::Borrowed(raw);
    }

    let normalized_num = normalize_css_number(num_part);
    let unit = canonical_unit(unit_part);

    // Both borrowed ⇒ nothing changed, so `num_part + unit_part == raw` (they are
    // the two halves of the same `split_at`): borrow the whole original slice.
    match (normalized_num, unit) {
        (Cow::Borrowed(_), Cow::Borrowed(_)) => Cow::Borrowed(raw),
        (num, unit) => Cow::Owned(format!("{num}{unit}")),
    }
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
///
/// Returns a `Cow`: an already-canonical number borrows the input slice (no
/// allocation), so a caller with the number's span can emit it verbatim. Any
/// rewrite yields `Cow::Owned` — including *every* number carrying an `e`/`E`
/// exponent, since exponents are rare in CSS and always-owning them keeps the
/// borrow invariant (`Cow::Borrowed` ⟺ output byte-identical to `num`) trivially
/// sound without a canonical-exponent check.
pub(crate) fn normalize_css_number(num: &str) -> Cow<'_, str> {
    let Some(e_idx) = num.find(['e', 'E']) else {
        // No exponent: the number is exactly its mantissa, so its Cow is the whole
        // number's Cow (borrows `num` unchanged when already canonical).
        return normalize_decimal_preserving_prefix(num);
    };
    let mantissa = &num[..e_idx];
    let exponent = &num[e_idx + 1..];

    let normalized_mantissa = normalize_decimal_preserving_prefix(mantissa);

    let (exp_sign, exp_digits) = if let Some(rest) = exponent.strip_prefix('-') {
        ("-", rest)
    } else if let Some(rest) = exponent.strip_prefix('+') {
        ("", rest)
    } else {
        ("", exponent)
    };

    let trimmed_digits = exp_digits.trim_start_matches('0');
    if trimmed_digits.is_empty() {
        // Exponent is zero or absent (`5e0`, `5e-00`, `1e`) — drop it, keeping the
        // mantissa. The `e`/`E` is removed, so this is always a rewrite → own it.
        return Cow::Owned(normalized_mantissa.into_owned());
    }

    Cow::Owned(format!("{normalized_mantissa}e{exp_sign}{trimmed_digits}"))
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

/// Map a CSS dimension unit to its canonical case: **lowercase**, for every unit.
/// Units are ASCII case-insensitive (CSS Syntax 3) and serialize as lowercase — CSS
/// Values 4 §6.2 (`1Q` serializes as `1q`) and §7.3 (`hz` is the canonical
/// `<frequency>` unit; `1Hz` serializes as `1hz`). This lowercases the frequency units
/// `Hz`/`kHz` and the quarter-millimeter `Q` along with everything else (`10HZ`→`10hz`,
/// `10Q`→`10q`), a deliberate divergence from prettier — which upcases those three to
/// their prose spelling. See `docs/conformance_prettier.md` §"Unit serialization case"
/// and the `units_serialize_case_prettier_divergence` fixture.
///
/// An already-lowercase unit is canonical and borrows unchanged. An **unknown** unit
/// (not in [`CSS_UNITS`]) is left untouched, matching prettier (`10FOO` stays `10FOO`).
pub(crate) fn canonical_unit(unit: &str) -> Cow<'_, str> {
    if !unit.bytes().any(|b| b.is_ascii_uppercase()) {
        return Cow::Borrowed(unit);
    }
    // Mixed/upper input: canonicalize only a known unit (prettier leaves unknown ones).
    if !is_known_css_unit(unit) {
        return Cow::Borrowed(unit);
    }
    Cow::Owned(unit.to_ascii_lowercase())
}

/// Normalize a decimal number while preserving sign and leading zeros.
///
/// Examples:
/// - `01.50` → `01.5` (preserve leading zero, trim trailing)
/// - `+10.0` → `+10` (preserve sign, trim trailing)
/// - `-0.0` → `-0` (preserve negative zero)
/// - `.5` → `0.5` (add leading zero)
///
/// Returns `Cow::Borrowed(num)` when the number is already canonical (no
/// leading-zero insertion and nothing to trim), so a canonical number allocates
/// nothing; any rewrite yields `Cow::Owned`.
fn normalize_decimal_preserving_prefix(num: &str) -> Cow<'_, str> {
    // Split off a sign; it is re-emitted unchanged (a borrowed sub-slice either way).
    let (sign, rest) = if let Some(stripped) = num.strip_prefix('-') {
        ("-", stripped)
    } else if let Some(stripped) = num.strip_prefix('+') {
        ("+", stripped)
    } else {
        ("", num)
    };

    // A bare `.5` needs a leading zero (`.5` → `0.5`).
    let needs_leading_zero = rest.starts_with('.');

    // Trailing-zero / trailing-dot trimming applies only to a fractional part.
    // Popping trailing `0`s can never remove the dot (a dot isn't a `0`), so this
    // matches the original loop that trimmed only while a `.` was still present.
    let bytes = rest.as_bytes();
    let mut trim_to = rest.len();
    if rest.contains('.') {
        while trim_to > 0 && bytes[trim_to - 1] == b'0' {
            trim_to -= 1;
        }
        if trim_to > 0 && bytes[trim_to - 1] == b'.' {
            trim_to -= 1;
        }
    }

    // Identity fast path: no leading zero to insert and nothing trimmed ⇒ the
    // canonical form equals `num`, so borrow it.
    if !needs_leading_zero && trim_to == rest.len() {
        return Cow::Borrowed(num);
    }

    let leading = if needs_leading_zero { "0" } else { "" };
    Cow::Owned(format!("{sign}{leading}{}", &rest[..trim_to]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_unit() {
        // Every known unit lowercases (the spec's serialized form).
        assert_eq!(canonical_unit("PX"), "px");
        assert_eq!(canonical_unit("DEG"), "deg");
        assert_eq!(canonical_unit("Turn"), "turn");
        assert_eq!(canonical_unit("FR"), "fr");
        // Already-canonical borrows unchanged.
        assert!(matches!(canonical_unit("px"), Cow::Borrowed("px")));
        // `Hz`/`kHz`/`Q` lowercase too (CSS Values 4 §6.2/§7.3 — diverges from
        // prettier, which upcases them). Already-lowercase forms borrow unchanged.
        assert_eq!(canonical_unit("HZ"), "hz");
        assert_eq!(canonical_unit("KHZ"), "khz");
        assert_eq!(canonical_unit("Q"), "q");
        assert!(matches!(canonical_unit("hz"), Cow::Borrowed("hz")));
        assert!(matches!(canonical_unit("khz"), Cow::Borrowed("khz")));
        assert!(matches!(canonical_unit("q"), Cow::Borrowed("q")));
        // Unknown units are left untouched (prettier does the same).
        assert_eq!(canonical_unit("FOO"), "FOO");
        assert_eq!(canonical_unit(""), "");
    }

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

    #[test]
    fn test_normalize_css_number_borrows_when_canonical() {
        // An already-canonical number borrows its input slice (zero allocation) —
        // the invariant `build_dimension_doc` relies on to emit a `source_span`.
        for canonical in ["0", "5", "123", "10", "0.5", "1.5", "00.5", "-0", "+10"] {
            assert!(
                matches!(normalize_css_number(canonical), Cow::Borrowed(_)),
                "canonical {canonical:?} must borrow"
            );
        }
        // Anything requiring a rewrite (leading `.`, trailing zeros/dot, any
        // exponent) allocates a fresh `Cow::Owned`.
        for rewritten in [".5", "5.", "1.50", "-0.0", "5e0", "1e+0010", "1.5E-3", "1e"] {
            assert!(
                matches!(normalize_css_number(rewritten), Cow::Owned(_)),
                "rewritten {rewritten:?} must own"
            );
        }
    }

    #[test]
    fn test_normalize_dimension_borrows_when_canonical() {
        // A canonical number + canonical (already-lowercase or absent) unit borrows
        // the whole `raw` slice, so the caller emits it as a verbatim source span.
        for canonical in ["10px", "0.5rem", "100", "1.5", "0", "-0", "+10em"] {
            assert!(
                matches!(normalize_dimension_from_source(canonical), Cow::Borrowed(_)),
                "canonical dimension {canonical:?} must borrow"
            );
        }
        // A rewritten number (`.5`→`0.5`) or an uppercase known unit (`PX`→`px`)
        // forces an owned rebuild.
        for rewritten in [".5px", "1.50rem", "10PX", "5e0"] {
            assert!(
                matches!(normalize_dimension_from_source(rewritten), Cow::Owned(_)),
                "rewritten dimension {rewritten:?} must own"
            );
        }
        // A bare identifier (no numeric part) is left untouched → borrowed.
        assert!(matches!(
            normalize_dimension_from_source("auto"),
            Cow::Borrowed("auto")
        ));
    }
}
