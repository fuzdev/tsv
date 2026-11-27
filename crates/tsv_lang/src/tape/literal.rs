//! A wire literal's tape meaning, read at compile time: what a writer's constant JSON fragment
//! (`,"optional":false,"typeAnnotation":`) says on the tape — a record opening, a field present,
//! a boolean, a list's brackets — as a short list of [`LitOp`]s.
//!
//! A tape copy's site hands its JSON literal to [`literal_ops`] in a `const` (through its site
//! macro), so the literal is the site's one statement of its bytes: the JSON copy writes it, the
//! tape copy replays its ops with [`super::TapeWriter::t_lit`]. A literal holding a key names
//! the entry its keys belong to, and every key is resolved here: a key the entry lacks, or a key
//! in a literal that names no entry, fails the build — nothing about a literal is looked up by
//! name at run time. A keyword field's value the literal holds is resolved here too, to its
//! index in the field's vocabulary ([`LitOp::Keyword`]). Any other value is resolved against
//! the field its key declared ([`super::TapeWriter`]'s pending field), so `false` is a boolean
//! word where the field is a boolean and a tag where it is a tagged value.
//!
//! The reader takes the writers' literals, not JSON in general: compact, escape-free strings, no
//! number but an unsigned integer, and a `type`-less object only under a key whose field names
//! its entry. One split is read: a string value whose text the site writes between two literals
//! (`,"operator":"` … `","prefix":true`) — a literal's trailing opening quote and its leading
//! closing one belong to that value, whose own data call writes it.

use super::{Enc, Field, NO_AUX, shapes};

/// One step of a literal on the tape. Module doc.
#[derive(Clone, Copy)]
pub enum LitOp {
    /// `{"type":"T"`: a typed record of the entry opens.
    Open(u16),
    /// `{` under a key whose field names a `type`-less entry: that entry's record opens.
    OpenObject(u16),
    /// `}`: the open record closes.
    Close,
    /// `"k":`: the field `k` of the entry the site names.
    Key {
        /// The entry the site names (the open record must be its record — checked as the
        /// tape is written, in every build).
        entry: u16,
        /// Its field `k`.
        field: Field,
    },
    /// A keyword field's value: its index in the field's vocabulary.
    Keyword {
        /// The vocabulary (the field's `aux`).
        vocab: u16,
        /// The word's index in it.
        index: u32,
    },
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
    /// A record: its entry, or `None` for one the literal did not open and no key of it has
    /// read yet (the site's named entry is its keys' record).
    Record(Option<u16>),
    /// A list.
    List,
}

const MAX_DEPTH: usize = 16;

/// The ops of literal `lit`, its keys belonging to entry `start` (`None`: a literal that holds
/// no key — one that does fails the build). `N` must be `literal_len(lit)`.
#[must_use]
pub const fn literal_ops<const N: usize>(start: Option<u16>, lit: &'static str) -> [LitOp; N] {
    let mut out = [LitOp::Close; N];
    assert!(
        scan(start, lit, &mut out, true) == N,
        "a literal's op count must be its `literal_len`"
    );
    out
}

/// How many ops literal `lit` has ([`literal_ops`]) — a count of its tokens alone, whatever
/// entry its keys belong to, so a site whose entry is a const generic sizes its ops without
/// naming it.
///
/// `#[inline]`, as every reader here is in effect: a writer evaluates it in a `const`, so no
/// build carries it as code — the attribute keeps the crate from emitting it (and the reader
/// beneath it) into every build that enables `_tape`, the bindings' among them.
#[must_use]
#[inline]
pub const fn literal_len(lit: &'static str) -> usize {
    scan(None, lit, &mut [], false)
}

/// The field of the one key literal `lit` holds, of entry `entry` — a stated key's
/// (`TapeWriter::t_key_stated`): a key whose value the next data call writes in its own
/// encoding (a record or a list it opens, a ref), so the tape states the key's presence bit with
/// no field pending. A literal that is not exactly one key, or a key of any other encoding,
/// fails the build — a keyword key's among them: its index's vocabulary must be the key's, which
/// only [`stated_keyword`] (`keyword!` / `keyword_key!`) ties, so a keyword key stated without it
/// is refused.
#[must_use]
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
pub const fn stated_key(entry: u16, lit: &'static str) -> Field {
    let field = one_key(entry, lit);
    match field.enc {
        Enc::Record | Enc::List | Enc::Ref => field,
        Enc::Keyword => panic!(
            "a keyword key is stated through `keyword!` (`stated_keyword`), which ties its index's vocabulary to the key"
        ),
        _ => panic!("a stated key's value writes its own encoding"),
    }
}

/// The field of the one keyword key literal `lit` holds, of entry `entry` — a stated key whose
/// value is a keyword index (`TapeWriter::t_keyword_id`), in the vocabulary [`stated_vocab`]
/// reads from the same literal: the path a keyword key is stated through (`keyword!` /
/// `keyword_key!`), so the key and its index's vocabulary are one build-time fact. A literal
/// that is not exactly one key, or a key of any other encoding, fails the build.
#[must_use]
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
pub const fn stated_keyword(entry: u16, lit: &'static str) -> Field {
    let field = one_key(entry, lit);
    match field.enc {
        Enc::Keyword => field,
        _ => panic!("a stated keyword key's field is a keyword field"),
    }
}

/// The vocabulary of the one keyword key literal `lit` holds, of entry `entry`
/// ([`stated_keyword`]'s field's): the site takes its keyword index's vocabulary from the key it
/// states, so the two cannot disagree. A literal that is not exactly one key, or a key of any
/// other encoding, fails the build.
#[must_use]
pub const fn stated_vocab(entry: u16, lit: &'static str) -> u16 {
    stated_keyword(entry, lit).aux
}

/// The field of the one key literal `lit` holds, of entry `entry`.
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
const fn one_key(entry: u16, lit: &'static str) -> Field {
    assert!(literal_len(lit) == 1, "a stated key's literal is one key");
    let ops: [LitOp; 1] = literal_ops(Some(entry), lit);
    match ops[0] {
        LitOp::Key { field, .. } => field,
        _ => panic!("a stated key's literal is one key"),
    }
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
/// returns the op count. `resolve`: look each name up (else count tokens only, naming nothing).
#[expect(
    clippy::panic,
    clippy::too_many_lines,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error; one \
              match over the literal's bytes"
)]
const fn scan(start: Option<u16>, lit: &'static str, out: &mut [LitOp], resolve: bool) -> usize {
    let bytes = lit.as_bytes();
    let mut levels = [Level::List; MAX_DEPTH];
    levels[0] = Level::Record(None);
    let mut depth = 1;
    // whether a key has taken the site's named entry
    let mut start_taken = false;
    // the field the last key declared, for a `type`-less `{` or a keyword value after it
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
                    let entry = if resolve {
                        match shapes::entry_named(slice(lit, from, to).as_bytes()) {
                            Some(entry) => entry,
                            None => panic!("no tape entry of that name"),
                        }
                    } else {
                        0
                    };
                    op = LitOp::Open(entry);
                    push(&mut levels, &mut depth, Level::Record(Some(entry)));
                    at = to + 1;
                } else {
                    let entry = match last {
                        Some(field) if field.aux != NO_AUX => field.aux,
                        _ if !resolve => 0,
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
                    let entry = match levels[depth - 1] {
                        Level::Record(Some(entry)) => entry,
                        // the site's named entry: the first key of a record the literal did
                        // not open
                        Level::Record(None) => match start {
                            Some(entry) if !start_taken => {
                                start_taken = true;
                                levels[depth - 1] = Level::Record(Some(entry));
                                entry
                            }
                            _ if !resolve => 0,
                            Some(_) => panic!("a literal's keys belong to two records it names"),
                            None => panic!("a literal holding a key must name its entry"),
                        },
                        Level::List => panic!("a key inside a list"),
                    };
                    let field = if resolve {
                        match shapes::field_named(entry, text.as_bytes()) {
                            Some(index) => shapes::ENTRIES[entry as usize].fields[index as usize],
                            None => panic!("the tape entry has no field of that key"),
                        }
                    } else {
                        Field {
                            key: "",
                            enc: Enc::Own,
                            bit: super::NO_BIT,
                            aux: NO_AUX,
                        }
                    };
                    last = Some(field);
                    op = LitOp::Key { entry, field };
                    at = to + 2;
                    if count < out.len() {
                        out[count] = op;
                    }
                    count += 1;
                    continue;
                }
                op = match last {
                    Some(Field {
                        enc: Enc::Keyword,
                        aux,
                        ..
                    }) if resolve => match shapes::keyword_named(aux, text.as_bytes()) {
                        Some(index) => LitOp::Keyword { vocab: aux, index },
                        None => panic!("the keyword is outside its field's vocabulary"),
                    },
                    _ => LitOp::Str(text),
                };
                at = to + 1;
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
        // a value, a record or a list ends the key's turn (a key, a comma or a split string quote `continue`s past this)
        last = None;
        if count < out.len() {
            out[count] = op;
        }
        count += 1;
    }
    count
}

/// Closes a level; closing past the literal's first level reaches a record the literal did not
/// open, whose keys are the site's named entry's.
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
    use crate::tape::{Enc, entry_id, field_aux, keyword_id};

    #[test]
    fn literal_reads_keys_values_and_records() {
        const E: Option<u16> = Some(entry_id("Identifier"));
        const S: &str = ",\"optional\":false,\"typeAnnotation\":";
        const OPS: [LitOp; literal_len(S)] = literal_ops(E, S);
        assert_eq!(OPS.len(), 3);
        assert!(
            matches!(OPS[0], LitOp::Key { field, .. } if field.key == "optional"
            && field.enc == Enc::Boolean)
        );
        assert!(matches!(OPS[1], LitOp::False));
        assert!(matches!(OPS[2], LitOp::Key { field, .. } if field.key == "typeAnnotation"));

        // a literal that closes a record it did not open: its key is the named entry's
        const TE: Option<u16> = Some(entry_id("TemplateElement"));
        const D: &str = "},\"tail\":true";
        const DOPS: [LitOp; literal_len(D)] = literal_ops(TE, D);
        assert!(matches!(
            DOPS,
            [LitOp::Close, LitOp::Key { entry, field }, LitOp::True]
                if Some(entry) == TE && field.key == "tail"
        ));

        const T: &str = "{\"type\":\"Literal\",\"value\":{}";
        const TOPS: [LitOp; literal_len(T)] = literal_ops(None, T);
        assert!(matches!(TOPS[0], LitOp::Open(e) if e == entry_id("Literal")));
        assert!(matches!(TOPS[2], LitOp::EmptyObject));

        // a keyword field's value resolves to its index
        const V: Option<u16> = Some(entry_id("VariableDeclaration"));
        const K: &str = ",\"kind\":\"let\"";
        const KOPS: [LitOp; literal_len(K)] = literal_ops(V, K);
        const VOCAB: u16 = field_aux(entry_id("VariableDeclaration"), "kind");
        assert!(matches!(
            KOPS[1],
            LitOp::Keyword { vocab, index } if vocab == VOCAB && index == keyword_id(VOCAB, "let")
        ));

        // the count reads no name
        assert_eq!(literal_len("}],\"x\":[]"), 5);
    }
}
