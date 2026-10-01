use crate::ast::internal::{AngleUnit, Color, ColorChannel};
use crate::color::is_hex_color_body;
use crate::escapes::trim_css;
use crate::keyword_set::ascii_keyword_set;

/// Parse a color value: hex, named, rgb(), hsl(), etc.
///
/// `Hex`/`Named` carry no text — their source is recovered from the value's span at
/// print time — so this only classifies and needs no arena.
pub fn parse_color(s: &str) -> Option<Color> {
    // Hex color: #RGB, #RGBA, #RRGGBB, or #RRGGBBAA. `is_hex_color_body` (shared with
    // the printer's value-text normalizer) also requires the body be all hex digits,
    // so a same-length non-hex hash (`#ZZZ`, `#12G`) stays a plain hash token — else
    // `format_color_from_source` would lowercase a non-color and the linter would
    // read it as one. The `#` is ASCII, so `strip_prefix` gives the exact body bytes.
    if s.strip_prefix('#').is_some_and(is_hex_color_body) {
        return Some(Color::Hex);
    }

    // Named color
    if is_named_color(s) {
        return Some(Color::Named);
    }

    None
}

/// Parse color function: rgb(r, g, b), rgba(r, g, b, a), hsl(...), hsla(...)
///
/// Only constructs the `Rgb`/`Hsl` variants. `name` is matched ASCII-case-insensitively —
/// CSS function names are ASCII case-insensitive, and this runs on *every* function value
/// the parser builds (`var`, `calc`, `clamp`, …), the overwhelming majority of which are
/// not color functions at all, so it must not cost an allocation to say "no".
pub fn parse_color_function(name: &str, args_str: &str) -> Option<Color> {
    if name.eq_ignore_ascii_case("rgb") || name.eq_ignore_ascii_case("rgba") {
        parse_rgb(args_str)
    } else if name.eq_ignore_ascii_case("hsl") || name.eq_ignore_ascii_case("hsla") {
        parse_hsl(args_str)
    } else {
        None
    }
}

/// Parse a color channel value: number, percentage, or "none"
///
/// Every trim in this file is [`trim_css`] — CSS whitespace only. A Unicode `str::trim`
/// would eat a non-ASCII space (an NBSP) glued to a channel and classify the value as a
/// color anyway, and the printer rebuilds a color from its parsed channels rather than from
/// source, so the character would be dropped. Left in the text, the space fails the channel
/// parse, the value stays a generic function, and its arguments print verbatim.
fn parse_color_channel(s: &str) -> Option<ColorChannel> {
    let s = trim_css(s);

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
    let s = trim_css(s);

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
    if let Ok(value) = trim_css(value_str).parse::<f64>() {
        return Some((ColorChannel::Number(value), unit));
    }

    None
}

/// The three channels of a color function plus its optional alpha — everything
/// `parse_rgb` and `parse_hsl` read. A color function has at most four parts, so
/// they travel as a tuple rather than a heap list, and "exactly three channels"
/// is a fact the type states instead of a length check each caller repeats.
type ColorArgs<'a> = (&'a str, &'a str, &'a str, Option<&'a str>);

/// Split a color function's argument string into its channel parts across the
/// CSS Color 4 syntaxes (css-color-4 §rgb()/§hsl()) — legacy `c, c, c[, a]`,
/// modern `c c c`, and the modern slash-alpha form `c c c / a`. Shared by
/// `parse_rgb` and `parse_hsl`, which differ only in how they parse each channel.
///
/// Returns `None` unless the arity matches one of those grammars **exactly**: 3
/// (modern), 3 or 4 (legacy comma), or 3-channels-then-slash-alpha. A malformed
/// value — too many channels, or a slash form missing a channel — is deliberately
/// **not** classified as a color, so it falls through to the generic function path,
/// which preserves it verbatim (matching prettier). Classifying it here instead
/// would reconstruct it lossily: dropping the trailing channel, reinterpreting a
/// slash-alpha as the missing channel, or inserting legacy commas.
///
/// Channels split on CSS whitespace (css-syntax-3 §whitespace: no U+000B), not
/// only U+0020, so an interior tab/newline classifies the same as a space would —
/// otherwise the same color, authored across two lines, could reformat to two
/// different fixed points (once as a misclassified generic function, once as a
/// color).
///
/// One walk over the bytes decides all three: it stops at the first `/`, and on the way
/// records the first three channels a `,`-or-whitespace split makes (and whether there is a
/// fourth) and the first three commas (and whether there is a fourth). Every byte it tests
/// is ASCII, so each boundary it records is a character boundary.
fn split_color_args(args_str: &str) -> Option<ColorArgs<'_>> {
    let args_str = trim_css(args_str);
    let bytes = args_str.as_bytes();
    // The channels as `(start, end)` byte ranges, and how many there are, counted past the
    // three recorded only far enough to say "more".
    let mut channels = [(0, 0); 3];
    let mut channel_count = 0;
    let mut commas = [0; 3];
    let mut comma_count = 0;
    // The start of the channel being walked, or `None` between channels.
    let mut channel_start = None;
    let mut slash = None;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'/' => {
                slash = Some(i);
                break;
            }
            b',' | b' ' | b'\t' | b'\n' | b'\x0C' | b'\r' => {
                if let Some(start) = channel_start.take() {
                    if channel_count < 3 {
                        channels[channel_count] = (start, i);
                    }
                    channel_count = (channel_count + 1).min(4);
                }
                if b == b',' {
                    if comma_count < 3 {
                        commas[comma_count] = i;
                    }
                    comma_count = (comma_count + 1).min(4);
                }
            }
            _ => {
                if channel_start.is_none() {
                    channel_start = Some(i);
                }
            }
        }
    }
    if let Some(start) = channel_start {
        if channel_count < 3 {
            channels[channel_count] = (start, slash.unwrap_or(bytes.len()));
        }
        channel_count = (channel_count + 1).min(4);
    }
    let channel = |(start, end): (usize, usize)| &args_str[start..end];
    if let Some(slash) = slash {
        // Modern slash-alpha form: exactly `c c c / a`. Bind the alpha to the
        // fourth slot; the caller's channel parse rejects a malformed alpha (a
        // second slash, extra tokens, or an empty one all fail to parse), so only
        // the head arity is checked here.
        if channel_count != 3 {
            return None; // other than three channels before the slash
        }
        let [c1, c2, c3] = channels.map(channel);
        Some((c1, c2, c3, Some(trim_css(&args_str[slash + 1..]))))
    } else if comma_count > 0 {
        // Legacy comma form: exactly `c, c, c` or `c, c, c, a`. Empties are kept,
        // so `rgb(1,,2)` still presents three parts (and then fails to parse the
        // empty channel).
        let part = |start: usize, end: usize| trim_css(&args_str[start..end]);
        match comma_count {
            2 => Some((
                part(0, commas[0]),
                part(commas[0] + 1, commas[1]),
                part(commas[1] + 1, bytes.len()),
                None,
            )),
            3 => Some((
                part(0, commas[0]),
                part(commas[0] + 1, commas[1]),
                part(commas[1] + 1, commas[2]),
                Some(part(commas[2] + 1, bytes.len())),
            )),
            // fewer than three, or more than four, comma-separated parts
            _ => None,
        }
    } else {
        // Modern space form: exactly `c c c` (its alpha uses the slash form above).
        if channel_count != 3 {
            return None; // other than three space-separated channels
        }
        let [c1, c2, c3] = channels.map(channel);
        Some((c1, c2, c3, None))
    }
}

/// Parse rgb() or rgba() color
///
/// Supports CSS Color 4:
/// - Old format: rgb(255, 0, 0), rgba(255, 0, 0, 0.5)
/// - New format: rgb(255 0 0), rgb(255 0 0 / 0.5)
/// - Percentages: rgb(100% 0% 0%), rgb(100% 0% 0% / 50%)
/// - None keyword: rgb(255 0 none)
fn parse_rgb(args_str: &str) -> Option<Color> {
    let (red, green, blue, alpha_part) = split_color_args(args_str)?;

    let r = parse_color_channel(red)?;
    let g = parse_color_channel(green)?;
    let b = parse_color_channel(blue)?;
    // An absent alpha is fine; a present one that will not parse sinks the color.
    let alpha = match alpha_part {
        Some(alpha) => Some(parse_color_channel(alpha)?),
        None => None,
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
    let (hue_part, saturation_part, lightness_part, alpha_part) = split_color_args(args_str)?;

    let (hue, hue_unit) = parse_hue(hue_part)?;
    let saturation = parse_color_channel(saturation_part)?;
    let lightness = parse_color_channel(lightness_part)?;
    // An absent alpha is fine; a present one that will not parse sinks the color.
    let alpha = match alpha_part {
        Some(alpha) => Some(parse_color_channel(alpha)?),
        None => None,
    };

    Some(Color::Hsl {
        hue,
        hue_unit,
        saturation,
        lightness,
        alpha,
    })
}

ascii_keyword_set! {
    /// The CSS named colors (148 standard + 5 keywords), compiled at build time.
    static NAMED_COLORS;

    /// Is `s` a named CSS color (ASCII-case-insensitive)?
    ///
    /// Hot: the value parser asks this of every identifier-ish leaf it builds, so almost
    /// every call is a `var` / `auto` / `solid` / `--custom-property` that one bucket load
    /// refuses — see `keyword_set`.
    fn is_named_color;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three-pass spelling [`split_color_args`] replaced — the split on `/`, the comma
    /// test, then a char-decoding split — kept as the model its one byte walk must reproduce.
    fn split_color_args_model(args_str: &str) -> Option<ColorArgs<'_>> {
        let args_str = trim_css(args_str);
        if let Some((head, alpha)) = args_str.split_once('/') {
            let mut channels = head
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|s| !s.is_empty());
            let c1 = channels.next()?;
            let c2 = channels.next()?;
            let c3 = channels.next()?;
            if channels.next().is_some() {
                return None;
            }
            Some((c1, c2, c3, Some(trim_css(alpha))))
        } else if args_str.contains(',') {
            let mut parts = args_str.split(',').map(trim_css);
            let c1 = parts.next()?;
            let c2 = parts.next()?;
            let c3 = parts.next()?;
            let alpha = parts.next();
            if parts.next().is_some() {
                return None;
            }
            Some((c1, c2, c3, alpha))
        } else {
            let mut channels = args_str
                .split(|c: char| c.is_ascii_whitespace())
                .filter(|s| !s.is_empty());
            let c1 = channels.next()?;
            let c2 = channels.next()?;
            let c3 = channels.next()?;
            if channels.next().is_some() {
                return None;
            }
            Some((c1, c2, c3, None))
        }
    }

    /// Every argument string of up to five pieces over an alphabet of the separators each
    /// form splits on (`,`, `/` and the CSS whitespace, with the vertical tab that is not
    /// one), channel text (ASCII, non-ASCII, a trailing escape `trim_css` keeps) and the
    /// empty piece, split the same way — the same slices, not just equal text — as the
    /// three-pass model.
    #[test]
    fn color_args_split_as_the_model_splits_them() {
        let alphabet = [
            "", ",", "/", " ", "\t", "\n", "\r", "\u{c}", "\u{b}", "  ", "1", "50%", "0.5", "none",
            "é", "\u{a0}", "a\\ ", "deg",
        ];
        let mut graded = 0;
        let mut index = [0usize; 5];
        loop {
            let args: String = index.iter().map(|&i| alphabet[i]).collect();
            let ours = split_color_args(&args);
            let model = split_color_args_model(&args);
            let spans = |parts: Option<ColorArgs<'_>>| {
                parts.map(|(c1, c2, c3, alpha)| {
                    let at = |s: &str| (s.as_ptr() as usize - args.as_ptr() as usize, s.len());
                    (at(c1), at(c2), at(c3), alpha.map(at))
                })
            };
            assert_eq!(spans(ours), spans(model), "{args:?}");
            graded += 1;
            // Next index, odometer order.
            let mut digit = 0;
            loop {
                if digit == index.len() {
                    assert!(graded > 1_000_000, "{graded}");
                    return;
                }
                index[digit] += 1;
                if index[digit] < alphabet.len() {
                    break;
                }
                index[digit] = 0;
                digit += 1;
            }
        }
    }

    // The hex-body rule itself is exercised in `crate::color`; here we only confirm
    // `parse_color` routes a `#`-hash through it — a valid hex classifies, a
    // same-length non-hex hash does NOT (the linter-facing fact, invisible to
    // formatted output only for `Named`, but visible for `Hex` via lowercasing).
    #[test]
    fn hash_classification_requires_hex_digits() {
        assert!(matches!(parse_color("#ABC"), Some(Color::Hex)));
        assert!(matches!(parse_color("#AABBCCDD"), Some(Color::Hex)));
        assert!(parse_color("#ZZZ").is_none());
        assert!(parse_color("#12G").is_none());
    }
}
