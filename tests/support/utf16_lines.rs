//! The independent line reference the wire-coordinate tests grade `tsv_lang` against: a
//! line-start scan over a document's UTF-16 code units, sharing nothing with `tsv_lang`'s
//! line table or `WireCoordinates::point`. Every test that grades a line or a column
//! includes it by `#[path]` (`error_coordinates.rs`, `lexer_error_positions.rs`, and through
//! `support/loc_wire.rs` `loc_definition.rs` and `comment_dedent_document_line.rs`), so the
//! reference has one definition rather than one per test.

/// Which characters end a line in a document.
#[derive(Clone, Copy, Debug)]
pub enum LineRule {
    /// LF, CR, CRLF (one terminator), U+2028, U+2029 — a TypeScript document.
    Ecmascript,
    /// LF alone — a Svelte document and everything in it, and a CSS document.
    Lf,
}

/// How a language's wire reads a document, as `(line rule, elides a leading BOM)`: a
/// TypeScript document takes ECMAScript's terminators and counts a BOM (acorn reads it as
/// whitespace); a Svelte document — everything embedded in it included — and a CSS one take
/// LF alone and elide a BOM, as Svelte's `parse` and `parseCss` strip it before parsing.
pub const fn wire_rule(typescript: bool) -> (LineRule, bool) {
    if typescript {
        (LineRule::Ecmascript, false)
    } else {
        (LineRule::Lf, true)
    }
}

/// The text a wire's offsets index: `source` behind its leading BOM when the wire elides
/// one ([`wire_rule`]), `source` itself otherwise.
pub fn wire_text(source: &str, elide_bom: bool) -> &str {
    if elide_bom {
        source.strip_prefix('\u{feff}').unwrap_or(source)
    } else {
        source
    }
}

/// The unit offset each line of `units` starts at, line 1's (0) first.
pub fn line_starts(units: &[u16], rule: LineRule) -> Vec<usize> {
    let mut line_starts = vec![0];
    for (i, &unit) in units.iter().enumerate() {
        let ends_line = match rule {
            LineRule::Lf => unit == 0x0a,
            LineRule::Ecmascript => match unit {
                0x0a | 0x2028 | 0x2029 => true,
                // A CR followed by its LF is one terminator, ended by the LF.
                0x0d => units.get(i + 1) != Some(&0x0a),
                _ => false,
            },
        };
        if ends_line {
            line_starts.push(i + 1);
        }
    }
    line_starts
}

/// `(line, column)` of UTF-16 offset `offset` over `line_starts`: the line (1-based) is how
/// many lines start at or before it, the column (0-based) its distance from that line's
/// start.
pub fn line_column(line_starts: &[usize], offset: usize) -> (usize, usize) {
    let line = line_starts.partition_point(|&start| start <= offset);
    (line, offset - line_starts[line - 1])
}
