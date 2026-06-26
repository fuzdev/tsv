// Color formatting: semantic color rendering and source-preserving color syntax.

use crate::ast::internal::{Color, ColorChannel};
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
            // Format hue with optional unit
            let hue_str = if let Some(unit) = hue_unit {
                format!("{}{}", format_color_channel(hue), unit.as_str())
            } else {
                format_color_channel(hue)
            };
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

/// Format a color value with syntax preservation
///
/// Extracts the original syntax from source and reformats with proper spacing
/// while preserving the syntax choice (rgb vs rgba, comma vs space, / vs not).
///
/// # Arguments
/// * `color` - The parsed color
/// * `source` - The original source code
/// * `span` - The span of the color in source
pub(crate) fn format_color_from_source(color: &Color, source: &str, span: Span) -> String {
    // Named and hex colors are recovered verbatim from source (span-for-verbatim);
    // hex is lowercased to match prettier, named colors keep their source casing.
    match color {
        Color::Named => return span.extract(source).to_string(),
        Color::Hex => return span.extract(source).to_lowercase(),
        _ => {}
    }

    // Extract raw text to detect syntax
    let raw = span.extract(source);

    // Detect function name and syntax
    if let Some(open_paren) = raw.find('(') {
        let func_name = &raw[..open_paren];
        let has_slash = raw.contains('/');
        let has_comma = raw.contains(',');

        match color {
            Color::Rgb { r, g, b, alpha } => {
                let r_str = format_color_channel(r);
                let g_str = format_color_channel(g);
                let b_str = format_color_channel(b);

                if let Some(a) = alpha {
                    let a_str = format_color_channel(a);
                    if has_slash {
                        // Preserve original function name with slash syntax
                        format!("{func_name}({r_str} {g_str} {b_str} / {a_str})")
                    } else {
                        // Preserve original function name with comma syntax
                        format!("{func_name}({r_str}, {g_str}, {b_str}, {a_str})")
                    }
                } else if has_comma {
                    format!("{func_name}({r_str}, {g_str}, {b_str})")
                } else {
                    format!("{func_name}({r_str} {g_str} {b_str})")
                }
            }
            Color::Hsl {
                hue,
                hue_unit,
                saturation,
                lightness,
                alpha,
            } => {
                // Format hue with optional unit
                let hue_str = if let Some(unit) = hue_unit {
                    format!("{}{}", format_color_channel(hue), unit.as_str())
                } else {
                    format_color_channel(hue)
                };
                let sat_str = format_color_channel(saturation);
                let light_str = format_color_channel(lightness);

                if let Some(a) = alpha {
                    let a_str = format_color_channel(a);
                    if has_slash {
                        // Preserve original function name with slash syntax
                        format!("{func_name}({hue_str} {sat_str} {light_str} / {a_str})")
                    } else {
                        // Preserve original function name with comma syntax
                        format!("{func_name}({hue_str}, {sat_str}, {light_str}, {a_str})")
                    }
                } else if has_comma {
                    format!("{func_name}({hue_str}, {sat_str}, {light_str})")
                } else {
                    format!("{func_name}({hue_str} {sat_str} {light_str})")
                }
            }
            // Fallback for any other color types (future-proofing)
            _ => format_color_value(color),
        }
    } else {
        // Fallback to basic formatting
        format_color_value(color)
    }
}
