use crate::ast::internal::Color;
use phf::phf_set;

/// Parse a color value: hex, named, rgb(), hsl(), etc.
pub fn parse_color(s: &str) -> Option<Color> {
    // Hex color: #RGB or #RRGGBB
    if s.starts_with('#') && (s.len() == 4 || s.len() == 7) {
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

/// Parse rgb() or rgba() color
///
/// Supports both old format "r, g, b" and new format "r g b / a"
fn parse_rgb(args_str: &str) -> Option<Color> {
    let args_str = args_str.trim();
    let parts: Vec<&str> = if args_str.contains('/') {
        // New format: r g b / a
        let (rgb_part, alpha_part) = args_str.split_once('/')?;
        let mut parts = rgb_part
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(|s| s.trim())
            .collect::<Vec<_>>();
        parts.push(alpha_part.trim());
        parts
    } else {
        // Old format: r, g, b or r, g, b, a
        args_str.split(',').map(|s| s.trim()).collect::<Vec<_>>()
    };

    if parts.len() < 3 {
        return None;
    }

    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;
    let alpha = if parts.len() > 3 {
        parts[3].parse::<f64>().ok()
    } else {
        None
    };

    Some(Color::Rgb { r, g, b, alpha })
}

/// Parse hsl() or hsla() color
///
/// Supports both old format "h, s%, l%" and new format "h s% l% / a"
fn parse_hsl(args_str: &str) -> Option<Color> {
    let args_str = args_str.trim();
    let parts: Vec<&str> = if args_str.contains('/') {
        let (hsl_part, alpha_part) = args_str.split_once('/')?;
        let mut parts = hsl_part
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(|s| s.trim())
            .collect::<Vec<_>>();
        parts.push(alpha_part.trim());
        parts
    } else {
        args_str.split(',').map(|s| s.trim()).collect::<Vec<_>>()
    };

    if parts.len() < 3 {
        return None;
    }

    let hue = parts[0].trim_end_matches('%').parse::<f64>().ok()?;
    let saturation = parts[1].trim_end_matches('%').parse::<f64>().ok()?;
    let lightness = parts[2].trim_end_matches('%').parse::<f64>().ok()?;
    let alpha = if parts.len() > 3 {
        parts[3].trim_end_matches('%').parse::<f64>().ok()
    } else {
        None
    };

    Some(Color::Hsl {
        hue,
        saturation,
        lightness,
        alpha,
    })
}

/// Check if string is a named CSS color (case-insensitive, O(1) lookup)
fn is_named_color(s: &str) -> bool {
    NAMED_COLORS.contains(s.to_lowercase().as_str())
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
