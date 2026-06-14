use crate::ast::internal::{AngleUnit, Color, ColorChannel};
use phf::phf_set;

/// Parse a color value: hex, named, rgb(), hsl(), etc.
pub fn parse_color(s: &str) -> Option<Color> {
    // Hex color: #RGB, #RGBA, #RRGGBB, or #RRGGBBAA
    // Length includes the # prefix:
    // - 4: #RGB (3-digit)
    // - 5: #RGBA (4-digit with alpha)
    // - 7: #RRGGBB (6-digit)
    // - 9: #RRGGBBAA (8-digit with alpha)
    if s.starts_with('#') && matches!(s.len(), 4 | 5 | 7 | 9) {
        return Some(Color::Hex(s.to_string()));
    }

    // Named color
    if is_named_color(s) {
        return Some(Color::Named(s.to_string()));
    }

    None
}

/// Parse color function: rgb(r, g, b), rgba(r, g, b, a), hsl(...), hsla(...)
pub fn parse_color_function(name: &str, args_str: &str) -> Option<Color> {
    match name {
        "rgb" | "rgba" => parse_rgb(args_str),
        "hsl" | "hsla" => parse_hsl(args_str),
        _ => None,
    }
}

/// Parse a color channel value: number, percentage, or "none"
fn parse_color_channel(s: &str) -> Option<ColorChannel> {
    let s = s.trim();

    // CSS Color 4 "none" keyword
    if s.eq_ignore_ascii_case("none") {
        return Some(ColorChannel::None);
    }

    // Percentage: 50%, 100%, etc.
    if let Some(percent_str) = s.strip_suffix('%')
        && let Ok(value) = percent_str.parse::<f64>()
    {
        return Some(ColorChannel::Percentage(value));
    }

    // Numeric value: 255, 0.5, etc.
    if let Ok(value) = s.parse::<f64>() {
        return Some(ColorChannel::Number(value));
    }

    None
}

/// Parse hue value with optional angle unit
fn parse_hue(s: &str) -> Option<(ColorChannel, Option<AngleUnit>)> {
    let s = s.trim();

    // Check for "none" keyword
    if s.eq_ignore_ascii_case("none") {
        return Some((ColorChannel::None, None));
    }

    // Try to extract angle unit
    let (value_str, unit) = if s.ends_with("deg") {
        (s.trim_end_matches("deg"), Some(AngleUnit::Deg))
    } else if s.ends_with("rad") {
        (s.trim_end_matches("rad"), Some(AngleUnit::Rad))
    } else if s.ends_with("turn") {
        (s.trim_end_matches("turn"), Some(AngleUnit::Turn))
    } else if s.ends_with("grad") {
        (s.trim_end_matches("grad"), Some(AngleUnit::Grad))
    } else {
        // No unit = unitless number (treated as degrees)
        (s, None)
    };

    // Parse numeric value
    if let Ok(value) = value_str.trim().parse::<f64>() {
        return Some((ColorChannel::Number(value), unit));
    }

    None
}

/// Split a color function's argument string into its channel parts across the
/// CSS Color 4 syntaxes — `c, c, c[, a]`, space-separated `c c c`, and the
/// slash-alpha form `c c c / a`. Returns `None` if fewer than 3 parts. Shared by
/// `parse_rgb` and `parse_hsl`, which differ only in how they parse each channel.
fn split_color_args(args_str: &str) -> Option<Vec<&str>> {
    let args_str = args_str.trim();
    let parts: Vec<&str> = if let Some((head, alpha_part)) = args_str.split_once('/') {
        // Slash-alpha form: c c c / a
        let mut parts = head
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(str::trim)
            .collect::<Vec<_>>();
        parts.push(alpha_part.trim());
        parts
    } else if args_str.contains(',') {
        // Comma-separated: c, c, c[, a]
        args_str.split(',').map(str::trim).collect::<Vec<_>>()
    } else {
        // Space-separated: c c c
        args_str
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(str::trim)
            .collect::<Vec<_>>()
    };

    (parts.len() >= 3).then_some(parts)
}

/// Parse rgb() or rgba() color
///
/// Supports CSS Color 4:
/// - Old format: rgb(255, 0, 0), rgba(255, 0, 0, 0.5)
/// - New format: rgb(255 0 0), rgb(255 0 0 / 0.5)
/// - Percentages: rgb(100% 0% 0%), rgb(100% 0% 0% / 50%)
/// - None keyword: rgb(255 0 none)
fn parse_rgb(args_str: &str) -> Option<Color> {
    let parts = split_color_args(args_str)?;

    let r = parse_color_channel(parts[0])?;
    let g = parse_color_channel(parts[1])?;
    let b = parse_color_channel(parts[2])?;
    let alpha = if parts.len() > 3 {
        Some(parse_color_channel(parts[3])?)
    } else {
        None
    };

    Some(Color::Rgb { r, g, b, alpha })
}

/// Parse hsl() or hsla() color
///
/// Supports CSS Color 4:
/// - Old format: hsl(0, 100%, 50%), hsla(0, 100%, 50%, 0.5)
/// - New format: hsl(0 100% 50%), hsl(0 100% 50% / 0.5)
/// - Angle units: hsl(120deg 75% 25%), hsl(1.57rad 50% 50%)
/// - None keyword: hsl(none 50% 50%)
/// - Alpha as percentage: hsl(0 100% 50% / 50%)
fn parse_hsl(args_str: &str) -> Option<Color> {
    let parts = split_color_args(args_str)?;

    let (hue, hue_unit) = parse_hue(parts[0])?;
    let saturation = parse_color_channel(parts[1])?;
    let lightness = parse_color_channel(parts[2])?;
    let alpha = if parts.len() > 3 {
        Some(parse_color_channel(parts[3])?)
    } else {
        None
    };

    Some(Color::Hsl {
        hue,
        hue_unit,
        saturation,
        lightness,
        alpha,
    })
}

/// Check if string is a named CSS color (case-insensitive, O(1) lookup)
fn is_named_color(s: &str) -> bool {
    // Fast path: names arrive lowercase, so probe the set directly and only
    // allocate a lowercased copy when the input actually has uppercase ASCII.
    NAMED_COLORS.contains(s)
        || (s.bytes().any(|b| b.is_ascii_uppercase())
            && NAMED_COLORS.contains(s.to_ascii_lowercase().as_str()))
}

/// Static set of CSS named colors (148 standard + 5 keywords) - compiled at build time
static NAMED_COLORS: phf::Set<&'static str> = phf_set! {
    // Standard colors
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgrey",
    "darkgreen",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "grey",
    "green",
    "greenyellow",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgrey",
    "lightgreen",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
    // Keywords
    "currentcolor",
    "transparent",
    "inherit",
    "initial",
    "unset",
};
