//! A wire literal's tape meaning, read at compile time: what a writer's constant JSON fragment
//! (`,"optional":false,"typeAnnotation":`) says on the tape — a record opening, a field present,
//! a boolean, a list's brackets — as a short list of [`LitOp`]s.
//!
//! A tape copy's site hands its JSON literal to [`literal_ops`] in a `const` (through its site
//! macro), so the literal is the site's one statement of its bytes: the JSON copy writes it, the
//! tape copy replays its ops with [`super::TapeWriter::t_lit`]. When the site names the record
//! its keys belong to, every key is resolved here, and a key the entry lacks fails the build;
//! a site shared by several entries leaves the record to run time ([`LitOp::KeyNamed`]). A value
//! is resolved against the field its key declared ([`super::TapeWriter`]'s pending field), so
//! `false` is a boolean word where the field is a boolean and a tag where it is a tagged value.
//!
//! The reader takes the writers' literals, not JSON in general: compact, escape-free strings, no
//! number but an unsigned integer, and a `type`-less object only under a key whose field names
//! its entry. One split is read: a string value whose text the site writes between two literals
//! (`,"operator":"` … `","prefix":true`) — a literal's trailing opening quote and its leading
//! closing one belong to that value, whose own data call writes it.

use super::{Field, NO_AUX, shapes};

/// One step of a literal on the tape. Module doc.
#[derive(Clone, Copy)]
pub enum LitOp {
    /// `{"type":"T"`: a typed record of the entry opens.
    Open(u16),
    /// `{` under a key whose field names a `type`-less entry: that entry's record opens.
    OpenObject(u16),
    /// `}`: the open record closes.
    Close,
    /// `"k":` of a record the literal knows: the entry's field.
    Key {
        /// The entry the site names (checked against the open record in debug builds).
        entry: u16,
        /// Its field `k`.
        field: Field,
    },
    /// `"k":` of whatever record is open, looked up at run time.
    KeyNamed(&'static str),
    /// `true`.
    True,
    /// `false`.
    False,
    /// `null`.
    Null,
    /// `{}`.
    EmptyObject,
    /// A string value.
    Str(&'static str),
    /// An unsigned integer value.
    Int(u32),
    /// `[`.
    ListOpen,
    /// `]`.
    ListClose,
}

/// What the reader knows about one open level of the literal.
#[derive(Clone, Copy)]
enum Level {
    /// A record: its entry, or `None` when it is left to run time.
    Record(Option<u16>),
    /// A list.
    List,
}

const MAX_DEPTH: usize = 16;

/// The ops of literal `lit`, its first keys belonging to entry `start` (`None`: whatever record
/// is open at run time). `N` must be `literal_len(start, lit)`.
#[must_use]
pub const fn literal_ops<const N: usize>(start: Option<u16>, lit: &'static str) -> [LitOp; N] {
    let mut out = [LitOp::Close; N];
    assert!(
        scan(start, lit, &mut out) == N,
        "a literal's op count must be its `literal_len`"
    );
    out
}

/// How many ops literal `lit` has ([`literal_ops`]).
///
/// `#[inline]`, as every reader here is in effect: a writer evaluates it in a `const`, so no
/// build carries it as code — the attribute keeps the crate from emitting it (and the reader
/// beneath it) into every build that enables `_tape`, the bindings' among them.
#[must_use]
#[inline]
pub const fn literal_len(start: Option<u16>, lit: &'static str) -> usize {
    scan(start, lit, &mut [])
}

/// `lit[from..to]`, borrowed for `'static` (the literal is ASCII-checked by [`scan`]).
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
const fn slice(lit: &'static str, from: usize, to: usize) -> &'static str {
    let (_, rest) = lit.as_bytes().split_at(from);
    let (mid, _) = rest.split_at(to - from);
    match core::str::from_utf8(mid) {
        Ok(text) => text,
        Err(_) => panic!("a literal's strings are ASCII"),
    }
}

/// Opens a level.
const fn push(levels: &mut [Level; MAX_DEPTH], depth: &mut usize, level: Level) {
    assert!(*depth < MAX_DEPTH, "a literal nests too deep");
    levels[*depth] = level;
    *depth += 1;
}

/// Reads `lit` into `out` (when it is long enough to hold every op; `&mut []` only counts) and
/// returns the op count.
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
const fn scan(start: Option<u16>, lit: &'static str, out: &mut [LitOp]) -> usize {
    let bytes = lit.as_bytes();
    let mut levels = [Level::List; MAX_DEPTH];
    levels[0] = Level::Record(start);
    let mut depth = 1;
    // the field the last key declared, for a `type`-less `{` after it
    let mut last: Option<Field> = None;
    let mut count = 0;
    let mut at = 0;
    while at < bytes.len() {
        let op;
        match bytes[at] {
            b',' => {
                at += 1;
                continue;
            }
            b'{' => {
                if at + 1 < bytes.len() && bytes[at + 1] == b'}' {
                    op = LitOp::EmptyObject;
                    at += 2;
                } else if starts_with_at(bytes, at + 1, b"\"type\":\"") {
                    let from = at + 9;
                    let to = string_end(bytes, from);
                    let Some(entry) = shapes::entry_named(slice(lit, from, to).as_bytes()) else {
                        panic!("no tape entry of that name");
                    };
                    op = LitOp::Open(entry);
                    push(&mut levels, &mut depth, Level::Record(Some(entry)));
                    at = to + 1;
                } else {
                    let entry = match last {
                        Some(field) if field.aux != NO_AUX => field.aux,
                        _ => panic!("a type-less object opens under a key that names no entry"),
                    };
                    op = LitOp::OpenObject(entry);
                    push(&mut levels, &mut depth, Level::Record(Some(entry)));
                    at += 1;
                }
            }
            b'}' => {
                op = LitOp::Close;
                pop(&mut levels, &mut depth);
                at += 1;
            }
            b'[' => {
                op = LitOp::ListOpen;
                push(&mut levels, &mut depth, Level::List);
                at += 1;
            }
            b']' => {
                op = LitOp::ListClose;
                pop(&mut levels, &mut depth);
                at += 1;
            }
            // a string value's quotes split across two literals, its text a data call between
            // them (an operator written between the quotes): the closing quote at a literal's
            // start and the opening one at its end, after a key, are the value's, not a string
            b'"' if at == 0 && (bytes.len() == 1 || matches!(bytes[1], b',' | b'}' | b']')) => {
                at += 1;
                continue;
            }
            b'"' if at + 1 == bytes.len() && at > 0 && bytes[at - 1] == b':' => {
                at += 1;
                continue;
            }
            b'"' => {
                let from = at + 1;
                let to = string_end(bytes, from);
                let text = slice(lit, from, to);
                if to + 1 < bytes.len() && bytes[to + 1] == b':' {
                    op = match levels[depth - 1] {
                        Level::Record(Some(entry)) => {
                            let Some(index) = shapes::field_named(entry, text.as_bytes()) else {
                                panic!("the tape entry has no field of that key");
                            };
                            let field = shapes::ENTRIES[entry as usize].fields[index as usize];
                            last = Some(field);
                            LitOp::Key { entry, field }
                        }
                        Level::Record(None) => {
                            last = None;
                            LitOp::KeyNamed(text)
                        }
                        Level::List => panic!("a key inside a list"),
                    };
                    at = to + 2;
                } else {
                    op = LitOp::Str(text);
                    at = to + 1;
                }
            }
            b't' if starts_with_at(bytes, at, b"true") => {
                op = LitOp::True;
                at += 4;
            }
            b'f' if starts_with_at(bytes, at, b"false") => {
                op = LitOp::False;
                at += 5;
            }
            b'n' if starts_with_at(bytes, at, b"null") => {
                op = LitOp::Null;
                at += 4;
            }
            b'0'..=b'9' => {
                let mut value: u32 = 0;
                while at < bytes.len() && bytes[at].is_ascii_digit() {
                    value = value * 10 + (bytes[at] - b'0') as u32;
                    at += 1;
                }
                op = LitOp::Int(value);
            }
            _ => panic!("a byte the literal reader does not take"),
        }
        if count < out.len() {
            out[count] = op;
        }
        count += 1;
    }
    count
}

/// Closes a level; closing past the literal's first level leaves the record below to run time.
const fn pop(levels: &mut [Level; MAX_DEPTH], depth: &mut usize) {
    if *depth > 1 {
        *depth -= 1;
    } else {
        levels[0] = Level::Record(None);
    }
}

const fn starts_with_at(b: &[u8], at: usize, needle: &[u8]) -> bool {
    if at + needle.len() > b.len() {
        return false;
    }
    let mut k = 0;
    while k < needle.len() {
        if b[at + k] != needle[k] {
            return false;
        }
        k += 1;
    }
    true
}

/// The index of the `"` closing the string whose text starts at `from`.
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
const fn string_end(b: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < b.len() {
        match b[i] {
            b'"' => return i,
            b'\\' => panic!("a literal's strings are escape-free"),
            c if c >= 0x80 => panic!("a literal's strings are ASCII"),
            _ => i += 1,
        }
    }
    panic!("an unterminated string in a literal")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tape::{Enc, entry_id};

    #[test]
    fn literal_reads_keys_values_and_records() {
        const E: Option<u16> = Some(entry_id("Identifier"));
        const S: &str = ",\"optional\":false,\"typeAnnotation\":";
        const OPS: [LitOp; literal_len(E, S)] = literal_ops(E, S);
        assert_eq!(OPS.len(), 3);
        assert!(
            matches!(OPS[0], LitOp::Key { field, .. } if field.key == "optional"
            && field.enc == Enc::Boolean)
        );
        assert!(matches!(OPS[1], LitOp::False));
        assert!(matches!(OPS[2], LitOp::Key { field, .. } if field.key == "typeAnnotation"));

        const D: &str = "}],\"x\":[]";
        const DOPS: [LitOp; literal_len(None, D)] = literal_ops(None, D);
        assert!(matches!(
            DOPS,
            [
                LitOp::Close,
                LitOp::ListClose,
                LitOp::KeyNamed("x"),
                LitOp::ListOpen,
                LitOp::ListClose
            ]
        ));

        const T: &str = "{\"type\":\"Literal\",\"value\":{}";
        const TOPS: [LitOp; literal_len(None, T)] = literal_ops(None, T);
        assert!(matches!(TOPS[0], LitOp::Open(e) if e == entry_id("Literal")));
        assert!(matches!(TOPS[2], LitOp::EmptyObject));
    }
}
