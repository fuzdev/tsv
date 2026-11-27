//! The tape's debug side: [`tape_to_json`], which prints a tape back to the JSON wire it was
//! transcoded from, and [`fixture_wires`], every fixture document's span-only wire — what the
//! `tape` command streams to `check:tape` and what `tests/tape_roundtrip.rs` grades the
//! transcoder over.
//!
//! The printer is the transcoder's inverse and nothing ships it: `tape_to_json(transcode(json))`
//! equal to `json` byte for byte over every fixture document is the gate that the transcoder
//! lost nothing the decoder could not see (a number's spelling, a string's escapes). It writes
//! through `tsv_lang::JsonWriter`, the writers' own substrate, so its strings and numbers are
//! the writers' by construction, and reads the tape with an explicit frame stack, so it needs no
//! stack at any depth the transcoder accepts.

use crate::fixtures::validation::parsed_input::{ParsedInput, parse_input};
use crate::fixtures::{self, Fixture};
use std::path::Path;
use tsv_lang::JsonWriter;
use tsv_lang::tape::shapes::{ENTRIES, KEYWORDS, SHAPES_HASH, TAPE_FORMAT_VERSION};
use tsv_lang::tape::{
    Enc, HEADER_WORDS, Header, MAGIC, NO_BIT, NULL_REF, TAG_ARRAY, TAG_FALSE, TAG_NULL, TAG_NUMBER,
    TAG_OBJECT, TAG_STRING, TAG_TRUE, read_header,
};

/// The text a wire's offsets index: the source, less a leading byte-order mark for a Svelte or
/// CSS document (those wires elide it, as Svelte's `parse` and `parseCss` do).
#[must_use]
pub fn wire_source<'a>(language: &str, source: &'a str) -> &'a str {
    if language == "typescript" {
        source
    } else {
        source.strip_prefix('\u{FEFF}').unwrap_or(source)
    }
}

/// UTF-16 offsets → byte offsets over one text; `None` for an offset inside a pair or past it.
struct Utf16 {
    ascii: bool,
    len: usize,
    map: Vec<usize>,
}

impl Utf16 {
    fn new(text: &str) -> Self {
        if text.is_ascii() {
            return Self {
                ascii: true,
                len: text.len(),
                map: Vec::new(),
            };
        }
        let mut map = Vec::with_capacity(text.len() + 1);
        for (byte, ch) in text.char_indices() {
            map.push(byte);
            if ch.len_utf16() == 2 {
                map.push(usize::MAX);
            }
        }
        map.push(text.len());
        Self {
            ascii: false,
            len: text.len(),
            map,
        }
    }

    fn slice<'a>(&self, text: &'a str, start: u32, end: u32) -> Option<&'a str> {
        let (s, e) = if self.ascii {
            (start as usize, end as usize)
        } else {
            (*self.map.get(start as usize)?, *self.map.get(end as usize)?)
        };
        if s > e || e > self.len || s == usize::MAX || e == usize::MAX {
            return None;
        }
        text.get(s..e)
    }
}

/// An open container of the printed JSON.
enum Frame {
    Typed {
        entry: u16,
        mask: u32,
        field: usize,
        first: bool,
        start: u32,
        end: u32,
    },
    Escape {
        left: u32,
        first: bool,
    },
    Array {
        left: u32,
        first: bool,
        records: bool,
    },
}

/// What to read next.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Want {
    Record,
    Value,
}

struct Reader<'a> {
    words: &'a [u32],
    p: usize,
    source: &'a str,
    source_map: Utf16,
    pool: &'a str,
    pool_map: Utf16,
}

impl Reader<'_> {
    fn word(&mut self) -> Result<u32, String> {
        let w = *self
            .words
            .get(self.p)
            .ok_or_else(|| format!("tape: ends inside a record at word {}", self.p))?;
        self.p += 1;
        Ok(w)
    }

    fn string(&mut self) -> Result<Option<&str>, String> {
        let a = self.word()?;
        let b = self.word()?;
        if a == NULL_REF {
            return Ok(None);
        }
        let text = if a < NULL_REF {
            self.source_map.slice(self.source, a, b)
        } else {
            self.pool_map.slice(self.pool, a.wrapping_neg() - 1, b)
        };
        text.map(Some)
            .ok_or_else(|| format!("tape: a string ref [{a}, {b}] names no text"))
    }

    fn number(&mut self) -> Result<f64, String> {
        let lo = u64::from(self.word()?);
        let hi = u64::from(self.word()?);
        Ok(f64::from_bits(lo | (hi << 32)))
    }
}

/// Writes `n` as the writers do — JS `Number::toString`'s spelling: an integral value under
/// 1e21 as its integer digits, a non-integral one in the decade below 1e-5 in fixed notation,
/// any other through `JsonWriter::f64`. A restatement of `tsv_ts`'s `write_number_value`
/// (crate-private there), held to it by the round trip over every fixture.
fn write_number(w: &mut JsonWriter, n: f64) {
    if !n.is_finite() {
        w.null();
        return;
    }
    if n.fract() == 0.0 {
        if n.abs() < 9_007_199_254_740_992.0 {
            w.i64(n as i64);
            return;
        }
        let shortest = format!("{n}");
        if let Ok(v) = shortest.parse::<i64>() {
            w.i64(v);
            return;
        }
        if let Ok(v) = shortest.parse::<u64>() {
            w.u64(v);
            return;
        }
        if n.abs() < 1e21 {
            w.raw(&shortest);
            return;
        }
    } else if (1e-6..1e-5).contains(&n.abs()) {
        let sci = format!("{:e}", n.abs());
        let mantissa = sci.split('e').next().unwrap_or(&sci);
        let mut out = String::with_capacity(8 + mantissa.len());
        if n.is_sign_negative() {
            out.push('-');
        }
        out.push_str("0.00000");
        out.extend(mantissa.chars().filter(|&c| c != '.'));
        w.raw(&out);
        return;
    }
    w.f64(n);
}

/// Prints a tape back to the JSON it was transcoded from, against `source`, the text its offsets
/// index ([`wire_source`]).
///
/// # Errors
///
/// A message naming what is wrong when the bytes are not a tape of this schema.
pub fn tape_to_json(tape: &[u8], source: &str) -> Result<Vec<u8>, String> {
    let word_at = |i: usize| -> Option<u32> {
        let b = tape.get(i * 4..i * 4 + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let header: Vec<u32> = (0..HEADER_WORDS)
        .map(word_at)
        .collect::<Option<_>>()
        .ok_or("tape: shorter than its header")?;
    if header[0] != MAGIC || header[1] != TAPE_FORMAT_VERSION || header[2] != SHAPES_HASH {
        return Err("tape: another format, schema or shape list".to_owned());
    }
    let body = header[3] as usize;
    let pool_start = (HEADER_WORDS + body) * 4;
    if tape.len() != pool_start + header[4] as usize {
        return Err("tape: its length disagrees with its header".to_owned());
    }
    let words: Vec<u32> = (HEADER_WORDS..HEADER_WORDS + body)
        .map(word_at)
        .collect::<Option<_>>()
        .ok_or("tape: shorter than its body")?;
    let pool = std::str::from_utf8(&tape[pool_start..]).map_err(|e| format!("tape: pool: {e}"))?;
    if header[7] as usize != source.encode_utf16().count() {
        return Err("tape: written against another source (its UTF-16 length differs)".to_owned());
    }
    let mut r = Reader {
        words: &words,
        p: 0,
        source,
        source_map: Utf16::new(source),
        pool,
        pool_map: Utf16::new(pool),
    };
    let mut w = JsonWriter::with_capacity(tape.len() * 4);
    let mut stack: Vec<Frame> = Vec::new();
    let mut want = Want::Record;
    loop {
        // one read: a scalar written in place, or a container opened
        if want == Want::Record {
            match read_header(r.word()?).ok_or("tape: a header names no shape or entry")? {
                Header::Null => w.null(),
                Header::Escape => {
                    let left = r.word()?;
                    w.raw("{");
                    stack.push(Frame::Escape { left, first: true });
                }
                Header::Typed { entry, mask } => {
                    w.raw("{");
                    stack.push(Frame::Typed {
                        entry,
                        mask,
                        field: 0,
                        first: true,
                        start: 0,
                        end: 0,
                    });
                }
            }
        } else {
            match r.word()? {
                TAG_NULL => w.null(),
                TAG_FALSE => w.bool(false),
                TAG_TRUE => w.bool(true),
                TAG_NUMBER => write_number(&mut w, r.number()?),
                TAG_STRING => w.string(r.string()?.ok_or("tape: a null ref as a value")?),
                TAG_ARRAY => {
                    let left = r.word()?;
                    w.raw("[");
                    stack.push(Frame::Array {
                        left,
                        first: true,
                        records: false,
                    });
                }
                TAG_OBJECT => {
                    want = Want::Record;
                    continue;
                }
                tag => return Err(format!("tape: bad value tag {tag}")),
            }
        }
        // advance to the next nested read, closing each finished container
        loop {
            let Some(top) = stack.last_mut() else {
                if r.p != words.len() {
                    return Err(format!("tape: {} words left over", words.len() - r.p));
                }
                return Ok(w.into_bytes());
            };
            match top {
                Frame::Array {
                    left,
                    first,
                    records,
                } => {
                    if *left == 0 {
                        w.raw("]");
                        stack.pop();
                        continue;
                    }
                    *left -= 1;
                    if !std::mem::replace(first, false) {
                        w.raw(",");
                    }
                    want = if *records { Want::Record } else { Want::Value };
                    break;
                }
                Frame::Escape { left, first } => {
                    if *left == 0 {
                        w.raw("}");
                        stack.pop();
                        continue;
                    }
                    *left -= 1;
                    if !std::mem::replace(first, false) {
                        w.raw(",");
                    }
                    let key = r.string()?.ok_or("tape: a null ref as a key")?;
                    w.string(key);
                    w.raw(":");
                    want = Want::Value;
                    break;
                }
                Frame::Typed {
                    entry,
                    mask,
                    field,
                    first,
                    start,
                    end,
                } => {
                    let fields = ENTRIES[usize::from(*entry)].fields;
                    let mut nested = None;
                    let mut list = None;
                    while let Some(f) = fields.get(*field) {
                        *field += 1;
                        if f.bit != NO_BIT && *mask & (1 << f.bit) == 0 {
                            continue;
                        }
                        if !std::mem::replace(first, false) {
                            w.raw(",");
                        }
                        w.string(f.key);
                        w.raw(":");
                        match f.enc {
                            Enc::Type => w.string(ENTRIES[usize::from(*entry)].ty.unwrap_or("")),
                            Enc::Offset => {
                                let v = r.word()?;
                                if f.key == "start" {
                                    *start = v;
                                } else if f.key == "end" {
                                    *end = v;
                                }
                                w.u32(v);
                            }
                            Enc::Boolean => w.bool(r.word()? != 0),
                            Enc::Number => write_number(&mut w, r.number()?),
                            Enc::Null => w.null(),
                            Enc::Own => w.string(
                                r.source_map
                                    .slice(source, *start, *end)
                                    .ok_or("tape: an own string's span names no text")?,
                            ),
                            Enc::Keyword => {
                                let index = r.word()? as usize;
                                let values = KEYWORDS[usize::from(f.aux)];
                                w.string(values.get(index).ok_or("tape: no such keyword")?);
                            }
                            Enc::Ref | Enc::RefNullable => match r.string()? {
                                Some(s) => w.string(s),
                                None => w.null(),
                            },
                            Enc::Strings => {
                                let n = r.word()?;
                                w.raw("[");
                                for k in 0..n {
                                    if k > 0 {
                                        w.raw(",");
                                    }
                                    w.string(r.string()?.ok_or("tape: a null ref in a list")?);
                                }
                                w.raw("]");
                            }
                            Enc::Record => {
                                nested = Some(Want::Record);
                                break;
                            }
                            Enc::List => {
                                list = Some(r.word()?);
                                break;
                            }
                            Enc::Value => {
                                nested = Some(Want::Value);
                                break;
                            }
                        }
                    }
                    if let Some(left) = list {
                        // the loop steps the list next
                        w.raw("[");
                        stack.push(Frame::Array {
                            left,
                            first: true,
                            records: true,
                        });
                        continue;
                    }
                    if let Some(next) = nested {
                        want = next;
                        break;
                    }
                    w.raw("}");
                    stack.pop();
                }
            }
        }
    }
}

/// One fixture document's two parse wires.
#[derive(Debug)]
pub struct FixtureWire {
    /// The document's path under the walked root.
    pub path: String,
    /// `svelte`, `typescript` or `css` — the wire's language.
    pub language: &'static str,
    /// The exact text parsed (a leading byte-order mark included).
    pub source: String,
    /// The span-only wire JSON.
    pub span: Vec<u8>,
    /// The `loc`-bearing wire JSON of the same parse (`tsv parse --locations`).
    pub loc: Vec<u8>,
    /// The parse goal a TypeScript document was read at (the fixture's `goal` marker; `Module`
    /// for every other document).
    pub goal: tsv_ts::Goal,
}

/// Every parseable document under `root` the `loc_wires` walk grades, with both its wires:
/// each fixture's input and format variants; a prettier-side form tsv rejects is left out.
///
/// # Errors
///
/// A message when the tree can't be walked, or a document the fixture tree requires tsv to
/// parse can't be read or parsed.
pub fn fixture_wires(root: &Path, mut each: impl FnMut(FixtureWire)) -> Result<usize, String> {
    use crate::cli::commands::loc_wires::{Claim, documents};
    let all: Vec<Fixture> = fixtures::walk_fixtures(root)?;
    let mut n = 0;
    for fixture in &all {
        if fixture.tsv_rejects_path().exists() {
            continue;
        }
        let input_type = fixture.input_type();
        let goal = fixture.goal();
        for (name, claim) in documents(fixture) {
            let path = fixture.path.join(&name);
            let relative = format!("{}/{name}", fixture.relative_path);
            let source = fixtures::read_file(&path)?;
            let parsed = match parse_input(&source, input_type, goal) {
                Ok(parsed) => parsed,
                Err(_) if claim == Claim::PrettierSide => continue,
                Err(e) => return Err(format!("{relative}: {e}")),
            };
            let (language, span, loc) = match &parsed {
                ParsedInput::Svelte(ast) => (
                    "svelte",
                    tsv_svelte::__private::convert_ast_json_bytes(ast, &source),
                    tsv_svelte::__private::convert_ast_json_bytes_with_locations(ast, &source),
                ),
                ParsedInput::Ts(ast) => (
                    "typescript",
                    tsv_ts::__private::convert_ast_json_bytes(ast, &source),
                    tsv_ts::__private::convert_ast_json_bytes_with_locations(ast, &source),
                ),
                ParsedInput::Css(ast) => (
                    "css",
                    tsv_css::__private::convert_ast_json_bytes(ast, &source),
                    tsv_css::__private::convert_ast_json_bytes_with_locations(ast, &source),
                ),
            };
            each(FixtureWire {
                path: relative,
                language,
                source,
                span,
                loc,
                goal,
            });
            n += 1;
        }
    }
    Ok(n)
}

/// The CSS wire writer's DIRECT tape of `source` (`tsv_css::__private::write_tape`, the writer's
/// tape copy), graded against `json`, the same parse's span-only wire, by the two checks its
/// true-offset refs admit (docs/tape_format.md §String refs): it prints back to `json` byte for
/// byte, and the transcoder, as canonicalizer, makes the same tape of that printback as of
/// `json`. `Ok` is the direct tape, for a caller that decodes it too; `Err` names the failure.
///
/// # Errors
///
/// A message when the source does not parse, the sink refuses the tape, or either check fails.
pub fn grade_css_direct_tape(
    source: &str,
    json: &[u8],
    transcoder: &mut crate::transcoder::Transcoder,
    sink: &mut tsv_lang::tape::TapeSink,
) -> Result<Vec<u8>, String> {
    let ast = tsv_css::__private::parse(source).map_err(|e| format!("parse: {e:#}"))?;
    tsv_css::__private::write_tape(&ast, source, sink);
    grade_direct_tape(sink, wire_source("css", source), json, transcoder)
}

/// The TypeScript wire writer's DIRECT tape of `source` read at `goal`
/// (`tsv_ts::__private::write_tape`, the writer's tape copy), graded against `json`, the same
/// parse's span-only wire, by the two checks [`grade_css_direct_tape`] makes. `Ok` is the direct
/// tape; `Err` names the failure.
///
/// # Errors
///
/// A message when the source does not parse, the sink refuses the tape, or either check fails.
pub fn grade_ts_direct_tape(
    source: &str,
    goal: tsv_ts::Goal,
    json: &[u8],
    transcoder: &mut crate::transcoder::Transcoder,
    sink: &mut tsv_lang::tape::TapeSink,
) -> Result<Vec<u8>, String> {
    let ast =
        tsv_ts::__private::parse_with_goal(source, goal).map_err(|e| format!("parse: {e:#}"))?;
    tsv_ts::__private::write_tape(&ast, source, sink);
    grade_direct_tape(sink, wire_source("typescript", source), json, transcoder)
}

/// The Svelte wire writer's DIRECT tape of `source` (`tsv_svelte::__private::write_tape`, the
/// writer's tape copy, composing `tsv_ts`'s and `tsv_css`'s for the embedded islands), graded
/// against `json`, the same parse's span-only wire, by the two checks [`grade_css_direct_tape`]
/// makes. `Ok` is the direct tape; `Err` names the failure.
///
/// # Errors
///
/// A message when the source does not parse, the sink refuses the tape, or either check fails.
pub fn grade_svelte_direct_tape(
    source: &str,
    json: &[u8],
    transcoder: &mut crate::transcoder::Transcoder,
    sink: &mut tsv_lang::tape::TapeSink,
) -> Result<Vec<u8>, String> {
    let ast = tsv_svelte::__private::parse(source).map_err(|e| format!("parse: {e:#}"))?;
    tsv_svelte::__private::write_tape(&ast, source, sink);
    grade_direct_tape(sink, wire_source("svelte", source), json, transcoder)
}

/// The checks every direct tape is graded by, over the tape `sink` just took: its words (the
/// form a binding hands its host, `TapeSink::finish_words`) are its bytes, it prints back to
/// `json` byte for byte over `text` (the wire's source), and the transcoder, the oracle, makes
/// the same tape of that printback as of `json`.
///
/// # Errors
///
/// A message when the sink refuses the tape or either check fails.
pub fn grade_direct_tape(
    sink: &mut tsv_lang::tape::TapeSink,
    text: &str,
    json: &[u8],
    transcoder: &mut crate::transcoder::Transcoder,
) -> Result<Vec<u8>, String> {
    let direct = sink
        .finish_bytes()
        .map_err(|e| format!("the sink refused the direct tape: {e}"))?;
    // the bindings' form, the words laid out in place, is the same tape
    let (words, len) = sink
        .finish_words()
        .map_err(|e| format!("the sink refused the direct tape as words: {e}"))?;
    let laid: Vec<u8> = words.iter().flat_map(|w| w.to_ne_bytes()).collect();
    if words.len() != len.div_ceil(4)
        || laid.get(..len) != Some(direct.as_slice())
        || laid[len..].iter().any(|&b| b != 0)
    {
        return Err("the direct tape's words are not its bytes".to_owned());
    }
    let printed = tape_to_json(&direct, text).map_err(|e| format!("print: {e}"))?;
    if let Some(d) = bytes_difference(&printed, json) {
        return Err(format!("the direct tape prints back differently {d}"));
    }
    let canonical = transcoder
        .transcode(json, text)
        .map_err(|e| format!("transcode: {e}"))?
        .to_vec();
    let canonicalized = transcoder
        .transcode(&printed, text)
        .map_err(|e| format!("transcode of the printback: {e}"))?;
    if canonicalized != canonical.as_slice() {
        return Err("transcode(tape_to_json(direct)) differs from transcode(json)".to_owned());
    }
    Ok(direct)
}

/// The body of every `<script>` element in a Svelte `source`, each read as a TypeScript
/// document of its own — [`style_bodies`]'s twin. A body that is not standalone TypeScript (a
/// Svelte-only rune shape is still TypeScript; a `$`-label is too) just fails to parse.
#[must_use]
pub fn script_bodies(source: &str) -> Vec<&str> {
    element_bodies(source, "<script", "</script>")
}

/// The body of every `<style>` element in a Svelte `source` (the text between `<style …>` and
/// `</style>`), each read as a CSS document of its own — the CSS the fixture tree holds beyond
/// its few `.css` inputs. A plain text scan: a body that is not standalone CSS just fails to
/// parse, and its caller skips it.
#[must_use]
pub fn style_bodies(source: &str) -> Vec<&str> {
    element_bodies(source, "<style", "</style>")
}

/// The text between each `open …>` and the `close` after it.
fn element_bodies<'a>(source: &'a str, open_tag: &str, close_tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(open) = source[at..].find(open_tag) {
        let open = at + open;
        let Some(gt) = source[open..].find('>') else {
            break;
        };
        let body = open + gt + 1;
        let Some(close) = source[body..].find(close_tag) else {
            break;
        };
        out.push(&source[body..body + close]);
        at = body + close + close_tag.len();
    }
    out
}

/// Where two byte strings first differ, as the text around that byte on each side; `None` when
/// they are equal. A test's report of a printed tape against its wire.
#[must_use]
pub fn bytes_difference(a: &[u8], b: &[u8]) -> Option<String> {
    if a == b {
        return None;
    }
    let at = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()));
    let window = |s: &[u8]| {
        let from = at.saturating_sub(60).min(s.len());
        String::from_utf8_lossy(&s[from..(at + 60).min(s.len())]).into_owned()
    };
    Some(format!("at byte {at}: {} vs {}", window(a), window(b)))
}

/// Standard base64 (RFC 4648, padded) — how the `tape` command carries a tape on a JSON line.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * k)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcoder::Transcoder;

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (raw, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(raw.as_bytes()), encoded);
        }
    }

    #[test]
    fn a_self_describing_record_prints_back() {
        let json = r#"{"a":[1,-2.5,"x\n",null,true,{"b":{}}],"__proto__":"\u0001"}"#;
        let mut t = Transcoder::new();
        let tape = t.transcode(json.as_bytes(), "ab").unwrap().to_vec();
        assert_eq!(tape_to_json(&tape, "ab").unwrap(), json.as_bytes());
    }

    /// The transcoder's refit regression shape: objects that fail their entry at the close (a
    /// required `alternate` missing), nested, print back to themselves.
    #[test]
    fn nested_refits_print_back() {
        let mut json = String::from("null");
        for _ in 0..40 {
            json = format!(
                r#"{{"type":"IfStatement","start":0,"end":0,"test":{json},"consequent":null}}"#
            );
        }
        let mut t = Transcoder::new();
        let tape = t.transcode(json.as_bytes(), "").unwrap().to_vec();
        assert_eq!(tape_to_json(&tape, "").unwrap(), json.as_bytes());
    }

    #[test]
    fn a_tape_refuses_another_source() {
        let mut t = Transcoder::new();
        let tape = t.transcode(br#"{"a":"x"}"#, "x").unwrap().to_vec();
        assert!(tape_to_json(&tape, "\u{FEFF}x").is_err());
    }

    #[test]
    fn a_parse_wire_prints_back() {
        for source in [
            "let a = 'x\\u0041' + 1.5;",
            "/** é 😀 */ f(`a${b}c`, /re/g, 1e21, 0.000001);",
        ] {
            let ast = tsv_ts::__private::parse(source).unwrap();
            let json = tsv_ts::__private::convert_ast_json_bytes(&ast, source);
            let mut t = Transcoder::new();
            let tape = t.transcode(&json, source).unwrap().to_vec();
            let back = tape_to_json(&tape, source).unwrap();
            assert_eq!(bytes_difference(&back, &json), None, "{source}");
        }
    }
}
