// Fragment and text parsing

use crate::ast::internal::*;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

/// Decode HTML entities in text content
/// Handles:
/// - Named entities: &lt;, &gt;, &amp;, &quot;, &apos;, &nbsp;, etc.
/// - Numeric entities: &#160;, &#x00A0;
fn decode_html_entities(raw: &str) -> String {
    let mut result = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '&' {
            let mut entity = String::new();
            let mut is_hex = false;

            // Check for numeric entity: &#123; or &#x1A;
            if chars.peek() == Some(&'#') {
                chars.next(); // consume #

                if chars.peek() == Some(&'x') || chars.peek() == Some(&'X') {
                    chars.next(); // consume x/X
                    is_hex = true;
                }

                // Collect digits
                while let Some(&digit_ch) = chars.peek() {
                    if digit_ch == ';' {
                        chars.next(); // consume ;
                        break;
                    }
                    if digit_ch.is_ascii_alphanumeric() {
                        entity.push(digit_ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Try to decode numeric entity
                if !entity.is_empty()
                    && let Ok(code_point) = if is_hex {
                        u32::from_str_radix(&entity, 16)
                    } else {
                        entity.parse::<u32>()
                    }
                    && let Some(decoded_ch) = char::from_u32(code_point)
                {
                    result.push(decoded_ch);
                    continue;
                }

                // If numeric decode failed, output as-is
                result.push('&');
                result.push('#');
                if is_hex {
                    result.push('x');
                }
                result.push_str(&entity);
                result.push(';');
            } else {
                // Named entity: collect until semicolon
                while let Some(&entity_ch) = chars.peek() {
                    if entity_ch == ';' {
                        chars.next(); // consume ;
                        break;
                    }
                    if entity_ch.is_ascii_alphanumeric() {
                        entity.push(entity_ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Decode known named entities
                let decoded = match entity.as_str() {
                    "lt" => "<",
                    "gt" => ">",
                    "amp" => "&",
                    "quot" => "\"",
                    "apos" => "'",
                    "nbsp" => "\u{00A0}",
                    "copy" => "©",
                    "reg" => "®",
                    "deg" => "°",
                    "hellip" => "…",
                    "euro" => "€",
                    "mdash" => "—",
                    "ndash" => "–",
                    "lsquo" => "\u{2018}", // left single quote
                    "rsquo" => "\u{2019}", // right single quote
                    "ldquo" => "\u{201C}", // left double quote
                    "rdquo" => "\u{201D}", // right double quote
                    "times" => "×",
                    "divide" => "÷",
                    "sect" => "§",
                    "para" => "¶",
                    _ => {
                        // Unknown entity, output as-is
                        result.push('&');
                        result.push_str(&entity);
                        result.push(';');
                        continue;
                    }
                };
                result.push_str(decoded);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

impl<'a> SvelteParser<'a> {
    /// Parse text content between nodes
    ///
    /// The `raw` field stores the original text from source.
    /// The `data` field stores the decoded text with HTML entities converted.
    pub(crate) fn parse_text(&mut self, start: usize, end: usize) -> Result<Text, ParseError> {
        let raw = self.source[start..end].to_string();
        let data = decode_html_entities(&raw);
        Ok(Text {
            raw,
            data,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}
