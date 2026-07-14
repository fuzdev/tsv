// Color formatting: semantic color rendering and source-preserving color syntax.

use std::borrow::Cow;

use super::numbers::canonical_unit;
use crate::ast::internal::{AngleUnit, Color, ColorChannel};
use tsv_lang::Span;

/// Format a *computed* color value (`Rgb`/`Hsl`) semantically.
///
/// `Named`/`Hex` are not handled here — they carry no text and are rendered from
/// source by `format_color_from_source`.
pub(crate) fn format_color_value(color: &Color) -> String {
    match color {
        // `Named`/`Hex` carry no text — callers with a span use
        // `format_color_from_source`; this source-free path only handles the
        // computed `Rgb`/`Hsl` forms.
        #[allow(clippy::unreachable)] // callers route Named/Hex to format_color_from_source first
        Color::Named | Color::Hex => {
            unreachable!("named/hex colors are rendered from source via format_color_from_source")
        }
        Color::Rgb { r, g, b, alpha } => {
            let r_str = format_color_channel(r);
            let g_str = format_color_channel(g);
            let b_str = format_color_channel(b);

            if let Some(a) = alpha {
                let a_str = format_color_channel(a);
                format!("rgba({r_str}, {g_str}, {b_str}, {a_str})")
            } else {
                format!("rgb({r_str}, {g_str}, {b_str})")
            }
        }
        Color::Hsl {
            hue,
            hue_unit,
            saturation,
            lightness,
            alpha,
        } => {
            let hue_str = format_hue(hue, hue_unit.as_ref());
            let sat_str = format_color_channel(saturation);
            let light_str = format_color_channel(lightness);

            if let Some(a) = alpha {
                let a_str = format_color_channel(a);
                format!("hsla({hue_str}, {sat_str}, {light_str}, {a_str})")
            } else {
                format!("hsl({hue_str}, {sat_str}, {light_str})")
            }
        }
    }
}

/// Format a computed `f64` for CSS output: a whole number drops its fraction
/// (`1.0` → `1`), otherwise the default float rendering. (Distinct from
/// `normalize_css_number`, which canonicalizes a number's *source text*.)
fn format_css_f64(v: f64) -> String {
    if v.fract() == 0.0 {
        (v as i64).to_string()
    } else {
        v.to_string()
    }
}

/// Format a ColorChannel value
fn format_color_channel(channel: &ColorChannel) -> String {
    match channel {
        ColorChannel::Number(n) => format_css_f64(*n),
        ColorChannel::Percentage(p) => format!("{}%", format_css_f64(*p)),
        ColorChannel::None => "none".to_string(),
    }
}

/// Format an hsl hue, appending its canonicalized angle unit when present
/// (`180DEG` → `180deg`; a bare hue → just the number). Shared by the semantic
/// (`format_color_value`) and source-preserving (`format_color_from_source`) paths.
fn format_hue(hue: &ColorChannel, hue_unit: Option<&AngleUnit>) -> String {
    match hue_unit {
        Some(unit) => format!(
            "{}{}",
            format_color_channel(hue),
            canonical_unit(unit.as_str())
        ),
        None => format_color_channel(hue),
    }
}

/// Reassemble `name(c1 c2 c3)` in the source's separator syntax, detected on the raw
/// text: modern slash-alpha (`c1 c2 c3 / a`) and legacy comma (`c1, c2, c3[, a]`) are
/// the only ways an alpha is written, so a channel-only value keeps its space-or-comma
/// separator. Shared by `format_color_from_source`'s rgb and hsl arms, which differ
/// only in how they render their three channels.
fn format_color_syntax(
    func_name: &str,
    [c1, c2, c3]: [String; 3],
    alpha: Option<String>,
    has_slash: bool,
    has_comma: bool,
) -> String {
    match (alpha, has_slash) {
        (Some(a), true) => format!("{func_name}({c1} {c2} {c3} / {a})"),
        (Some(a), false) => format!("{func_name}({c1}, {c2}, {c3}, {a})"),
        (None, _) if has_comma => format!("{func_name}({c1}, {c2}, {c3})"),
        (None, _) => format!("{func_name}({c1} {c2} {c3})"),
    }
}

/// Format a color value with syntax preservation
///
/// Extracts the original syntax from source and reformats with proper spacing
/// while preserving the syntax choice (rgb vs rgba, comma vs space, / vs not).
///
/// # Arguments
/// * `color` - The parsed color
/// * `source` - The original source code
/// * `span` - The span of the color in source
pub(crate) fn format_color_from_source<'s>(
    color: &Color,
    source: &'s str,
    span: Span,
) -> Cow<'s, str> {
    // Named and hex colors are recovered verbatim from source (span-for-verbatim). A
    // named color keeps its source casing, so it borrows the slice unchanged and
    // `build_color_doc` emits it as a zero-allocation `DocText::SourceSpan` — like the
    // identifier / dimension paths. Hex borrows too when it is already lowercase (the
    // common case), owning a lowercased copy only when the source has uppercase A–F; the
    // function syntaxes below own their reconstructed text (genuine transforms).
    match color {
        Color::Named => return Cow::Borrowed(span.extract(source)),
        Color::Hex => {
            // A hex color is ASCII `#` + hex digits, so `to_lowercase` only rewrites A–F.
            // An uppercase-free slice is already canonical — borrow it verbatim (no alloc),
            // like the `canonical_unit` / `lowercase_property_name` siblings.
            let raw = span.extract(source);
            return if raw.bytes().any(|b| b.is_ascii_uppercase()) {
                Cow::Owned(raw.to_lowercase())
            } else {
                Cow::Borrowed(raw)
            };
        }
        _ => {}
    }

    // Extract raw text to detect syntax
    let raw = span.extract(source);

    // Detect function name and syntax
    if let Some(open_paren) = raw.find('(') {
        let func_name = &raw[..open_paren];
        let has_slash = raw.contains('/');
        let has_comma = raw.contains(',');

        // The rgb and hsl arms differ only in how they render their three channels;
        // `format_color_syntax` owns the shared separator logic (preserving the source's
        // slash / comma / space choice).
        Cow::Owned(match color {
            Color::Rgb { r, g, b, alpha } => format_color_syntax(
                func_name,
                [r, g, b].map(format_color_channel),
                alpha.as_ref().map(format_color_channel),
                has_slash,
                has_comma,
            ),
            Color::Hsl {
                hue,
                hue_unit,
                saturation,
                lightness,
                alpha,
            } => format_color_syntax(
                func_name,
                [
                    format_hue(hue, hue_unit.as_ref()),
                    format_color_channel(saturation),
                    format_color_channel(lightness),
                ],
                alpha.as_ref().map(format_color_channel),
                has_slash,
                has_comma,
            ),
            // Fallback for any other color types (future-proofing)
            _ => format_color_value(color),
        })
    } else {
        // Fallback to basic formatting
        Cow::Owned(format_color_value(color))
    }
}
