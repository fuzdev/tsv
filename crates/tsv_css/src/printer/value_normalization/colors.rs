// Color formatting: semantic color rendering and source-preserving color syntax.

use std::borrow::Cow;
use std::fmt::Write;

use super::numbers::canonical_unit;
use crate::ast::internal::{AngleUnit, Color, ColorChannel};
use tsv_lang::Span;

/// Format a *computed* color value (`Rgb`/`Hsl`) semantically.
///
/// `Named`/`Hex` are not handled here — they carry no text and are rendered from
/// source by `format_color_from_source`.
pub(crate) fn format_color_value(color: &Color) -> String {
    let mut out = String::new();
    match color {
        // `Named`/`Hex` carry no text — callers with a span use
        // `format_color_from_source`; this source-free path only handles the
        // computed `Rgb`/`Hsl` forms.
        #[allow(clippy::unreachable)] // callers route Named/Hex to format_color_from_source first
        Color::Named | Color::Hex => {
            unreachable!("named/hex colors are rendered from source via format_color_from_source")
        }
        // The source-free semantic form is always the legacy comma syntax with the
        // alpha-suffixed name (`rgba`/`hsla`); `write_color_syntax` with `has_comma`
        // and no slash renders exactly that.
        Color::Rgb { r, g, b, alpha } => write_color_syntax(
            &mut out,
            &ColorSyntax {
                func_name: if alpha.is_some() { "rgba" } else { "rgb" },
                has_slash: false,
                has_comma: true,
            },
            |o| write_color_channel(o, r),
            g,
            b,
            alpha.as_ref(),
        ),
        Color::Hsl {
            hue,
            hue_unit,
            saturation,
            lightness,
            alpha,
        } => write_color_syntax(
            &mut out,
            &ColorSyntax {
                func_name: if alpha.is_some() { "hsla" } else { "hsl" },
                has_slash: false,
                has_comma: true,
            },
            |o| write_hue(o, hue, hue_unit.as_ref()),
            saturation,
            lightness,
            alpha.as_ref(),
        ),
    }
    out
}

/// Append a *computed* `f64` for CSS output: a whole number drops its fraction
/// (`1.0` → `1`), otherwise the default float rendering. (Distinct from
/// `normalize_css_number`, which canonicalizes a number's *source text*.) Writes into
/// `out` rather than returning a `String`, so a color reconstruction allocates one buffer
/// for the whole function instead of one temporary per channel — the top CSS format-churn
/// cluster on real (color-heavy) stylesheets.
fn write_css_f64(out: &mut String, v: f64) {
    // `write!` to a `String` is infallible; the `Result` only satisfies `fmt::Write`.
    if v.fract() == 0.0 {
        let _ = write!(out, "{}", v as i64);
    } else {
        let _ = write!(out, "{v}");
    }
}

/// Append a `ColorChannel` value.
fn write_color_channel(out: &mut String, channel: &ColorChannel) {
    match channel {
        ColorChannel::Number(n) => write_css_f64(out, *n),
        ColorChannel::Percentage(p) => {
            write_css_f64(out, *p);
            out.push('%');
        }
        ColorChannel::None => out.push_str("none"),
    }
}

/// Append an hsl hue, with its canonicalized angle unit when present
/// (`180DEG` → `180deg`; a bare hue → just the number). Shared by the semantic
/// (`format_color_value`) and source-preserving (`format_color_from_source`) paths.
fn write_hue(out: &mut String, hue: &ColorChannel, hue_unit: Option<&AngleUnit>) {
    write_color_channel(out, hue);
    if let Some(unit) = hue_unit {
        out.push_str(&canonical_unit(unit.as_str()));
    }
}

/// How a color function is written in source: its name plus the separator style
/// (`has_slash` / `has_comma`), grouped so `write_color_syntax` takes one descriptor
/// rather than three positional flags. `format_color_value`'s source-free form builds one
/// with `has_comma` set (the legacy `rgba(…, …, …)` syntax).
struct ColorSyntax<'a> {
    func_name: &'a str,
    has_slash: bool,
    has_comma: bool,
}

/// Append `name(c1 c2 c3)` in the source's separator syntax, detected on the raw text:
/// modern slash-alpha (`c1 c2 c3 / a`) and legacy comma (`c1, c2, c3[, a]`) are the only
/// ways an alpha is written, so a channel-only value keeps its space-or-comma separator.
/// `write_c1` writes the first channel — rgb's plain `r`, or hsl's hue-with-unit — while
/// `c2`/`c3`/`alpha` are plain channels; the rgb and hsl arms differ only there. Building
/// straight into `out` avoids the per-channel `String`s the old `[String; 3]` form built.
fn write_color_syntax(
    out: &mut String,
    syntax: &ColorSyntax<'_>,
    write_c1: impl FnOnce(&mut String),
    c2: &ColorChannel,
    c3: &ColorChannel,
    alpha: Option<&ColorChannel>,
) {
    // Comma separators for legacy comma syntax and comma-alpha; space for the
    // channel-only-space and modern slash-alpha forms (mirrors the old match arms:
    // slash-alpha keeps space channels, comma-alpha uses commas, and with no alpha the
    // source's own comma-or-space choice is preserved).
    let comma = match (alpha.is_some(), syntax.has_slash) {
        (true, true) => false,
        (true, false) => true,
        (false, _) => syntax.has_comma,
    };
    let sep = if comma { ", " } else { " " };
    out.push_str(syntax.func_name);
    out.push('(');
    write_c1(out);
    out.push_str(sep);
    write_color_channel(out, c2);
    out.push_str(sep);
    write_color_channel(out, c3);
    if let Some(a) = alpha {
        out.push_str(if syntax.has_slash { " / " } else { ", " });
        write_color_channel(out, a);
    }
    out.push(')');
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
        // `write_color_syntax` owns the shared separator logic (preserving the source's
        // slash / comma / space choice) and builds straight into one buffer sized from the
        // raw slice — no per-channel `String`s.
        let syntax = ColorSyntax {
            func_name,
            has_slash,
            has_comma,
        };
        match color {
            Color::Rgb { r, g, b, alpha } => {
                let mut out = String::with_capacity(raw.len());
                write_color_syntax(
                    &mut out,
                    &syntax,
                    |o| write_color_channel(o, r),
                    g,
                    b,
                    alpha.as_ref(),
                );
                Cow::Owned(out)
            }
            Color::Hsl {
                hue,
                hue_unit,
                saturation,
                lightness,
                alpha,
            } => {
                let mut out = String::with_capacity(raw.len());
                write_color_syntax(
                    &mut out,
                    &syntax,
                    |o| write_hue(o, hue, hue_unit.as_ref()),
                    saturation,
                    lightness,
                    alpha.as_ref(),
                );
                Cow::Owned(out)
            }
            // Fallback for any other color types (future-proofing)
            _ => Cow::Owned(format_color_value(color)),
        }
    } else {
        // Fallback to basic formatting
        Cow::Owned(format_color_value(color))
    }
}
