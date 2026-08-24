//! Glob matching for a single ignore pattern, segment by segment.
//!
//! A pattern is parsed into a list of `Seg`ments split on `/`. A `**` segment
//! matches zero or more path segments; every other segment is a list of `Tok`s
//! that match within one path segment (never crossing `/`, which is why `*` and
//! character classes don't need to special-case the separator — segments are
//! already split). This mirrors the gitignore(5) pattern grammar.

/// One segment of a pattern.
#[derive(Debug)]
pub(crate) enum Seg {
    /// A literal `**` segment — matches zero or more path segments.
    DoubleStar,
    /// A normal segment, matched against one path segment via its tokens.
    Glob(Vec<Tok>),
}

/// A token within a single (non-`**`) segment.
#[derive(Debug)]
pub(crate) enum Tok {
    /// A literal character (escapes are resolved into this at parse time).
    Lit(char),
    /// `*` — matches zero or more characters within the segment.
    Star,
    /// `?` — matches exactly one character within the segment.
    Question,
    /// `[...]` — matches one character from (or not from) the set.
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
}

#[derive(Debug)]
pub(crate) enum ClassItem {
    Ch(char),
    Range(char, char),
}

/// A path segment, classified once per query as all-ASCII or not.
///
/// `match_segments` runs the same path against many rules across many ancestor
/// prefixes, so the inner glob match wants O(1) indexed character access. For an
/// ASCII segment — every segment of essentially every real path — the segment's
/// own bytes already *are* that array, so the flag buys the whole amortization
/// with no collection at all: one `is_ascii` word-scan per segment instead of
/// collecting a `Vec<char>` (a heap allocation, plus its `realloc` growth
/// chain, per segment per query). `text` also powers the cheap `&str` anchor
/// comparison in `Layer::relativize`.
#[derive(Debug)]
pub(crate) struct PathSeg<'a> {
    pub(crate) text: &'a str,
    /// Whether `text` is wholly ASCII, so a byte index is a character index.
    ascii: bool,
}

impl<'a> PathSeg<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self {
            text,
            ascii: text.is_ascii(),
        }
    }
}

impl Seg {
    /// The exact string this segment matches, or `None` if it carries any
    /// wildcard (`*`, `?`, `[...]`) or is a `**` segment. Used to resolve a
    /// rule's fixed leading path — the run of literal directory names before any
    /// glob — for the heuristic-shadow diagnostic (`IgnoreStack::has_negation_under`).
    pub(crate) fn literal(&self) -> Option<String> {
        match self {
            Seg::DoubleStar => None,
            Seg::Glob(toks) => toks
                .iter()
                .map(|t| match t {
                    Tok::Lit(c) => Some(*c),
                    Tok::Star | Tok::Question | Tok::Class { .. } => None,
                })
                .collect(),
        }
    }
}

/// Parses one pattern segment into its matcher form.
pub(crate) fn parse_segment(s: &str) -> Seg {
    if s == "**" {
        return Seg::DoubleStar;
    }
    let chars: Vec<char> = s.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                // backslash escapes the next char into a literal
                if i + 1 < chars.len() {
                    toks.push(Tok::Lit(chars[i + 1]));
                    i += 2;
                } else {
                    toks.push(Tok::Lit('\\'));
                    i += 1;
                }
            }
            '*' => {
                // collapse a run of `*` — inside a segment `**` is just `*`
                while i < chars.len() && chars[i] == '*' {
                    i += 1;
                }
                toks.push(Tok::Star);
            }
            '?' => {
                toks.push(Tok::Question);
                i += 1;
            }
            '[' => {
                if let Some((class, next)) = parse_class(&chars, i) {
                    toks.push(class);
                    i = next;
                } else {
                    // unterminated class — treat `[` as a literal
                    toks.push(Tok::Lit('['));
                    i += 1;
                }
            }
            c => {
                toks.push(Tok::Lit(c));
                i += 1;
            }
        }
    }
    Seg::Glob(toks)
}

/// Parses a `[...]` class starting at `open` (where `chars[open] == '['`).
/// Returns the class token and the index just past the closing `]`, or `None`
/// if the class is unterminated.
fn parse_class(chars: &[char], open: usize) -> Option<(Tok, usize)> {
    let mut i = open + 1;
    let mut negated = false;
    if i < chars.len() && (chars[i] == '!' || chars[i] == '^') {
        negated = true;
        i += 1;
    }
    let mut items = Vec::new();
    // a `]` right after `[` (or `[!`) is a literal `]`, not the terminator
    if i < chars.len() && chars[i] == ']' {
        items.push(ClassItem::Ch(']'));
        i += 1;
    }
    while i < chars.len() && chars[i] != ']' {
        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] != ']' {
            items.push(ClassItem::Range(chars[i], chars[i + 2]));
            i += 3;
        } else if chars[i] == '\\' && i + 1 < chars.len() {
            items.push(ClassItem::Ch(chars[i + 1]));
            i += 2;
        } else {
            items.push(ClassItem::Ch(chars[i]));
            i += 1;
        }
    }
    if i < chars.len() && chars[i] == ']' {
        Some((Tok::Class { negated, items }, i + 1))
    } else {
        None
    }
}

fn class_matches(negated: bool, items: &[ClassItem], c: char) -> bool {
    let hit = items.iter().any(|it| match it {
        ClassItem::Ch(x) => *x == c,
        ClassItem::Range(a, b) => *a <= c && c <= *b,
    });
    hit != negated
}

/// The `char` at byte offset `i` of a **non-ASCII** segment (`i` must be a char
/// boundary), paired with its UTF-8 width. Only this path decodes UTF-8; an
/// ASCII segment reads its byte inline in [`glob_seg_match`], which keeps that
/// loop indexing a hoisted byte slice — the shape the win depends on, since it
/// is what lets the bounds check and the stride fold away.
fn char_at(s: &str, i: usize) -> (char, usize) {
    // `i` is a char boundary, so this always yields; the fallback is unreachable
    // and, being a replacement char, could only fail to match anyway.
    let c = s[i..].chars().next().unwrap_or(char::REPLACEMENT_CHARACTER);
    (c, c.len_utf8())
}

/// Matches a pattern's segments against a path's segments, with `**` consuming
/// zero or more path segments.
pub(crate) fn match_segments(pat: &[Seg], path: &[PathSeg<'_>]) -> bool {
    match pat.split_first() {
        None => path.is_empty(),
        Some((Seg::DoubleStar, rest)) => {
            // `**` matches any number of leading path segments, then `rest` must
            // match the remainder. When `rest` carries no further `**` it has a
            // fixed segment count and can only align with the path's *tail*, so
            // jump straight there instead of trying every split point — the
            // floating `**/name` case (every plain pattern parses to a leading
            // `**`) was O(depth) match attempts against a deep path.
            if rest.iter().any(|s| matches!(s, Seg::DoubleStar)) {
                // another `**` ahead: fall back to the general split-point search
                // (`rest` is non-empty here, so `**` may still consume zero)
                (0..=path.len()).any(|i| match_segments(rest, &path[i..]))
            } else if rest.is_empty() {
                // a *trailing* `**` matches everything inside the anchor (≥ 1
                // segment), never the anchor dir itself — so a later `!foo/keep.ts`
                // can re-include the file (git's rule).
                !path.is_empty()
            } else {
                // fixed-length, `**`-free tail: it must match exactly the last
                // `rest.len()` segments (an interior `**` still consumed zero here,
                // so `a/**/b` keeps matching `a/b`).
                rest.len() <= path.len() && match_segments(rest, &path[path.len() - rest.len()..])
            }
        }
        Some((Seg::Glob(toks), rest)) => match path.split_first() {
            Some((head, tail)) if glob_seg_match(toks, head) => match_segments(rest, tail),
            _ => false,
        },
    }
}

/// Matches one path segment against a segment's tokens. Classic two-pointer
/// glob match with single-star backtracking; segments are short so this is
/// cheap.
///
/// Both cursors index *bytes*, but every advance steps one whole code point, so
/// `?`, `*` and `[...]` stay code-point granular — the granularity
/// `glob_is_code_point_granular` pins. `si` is therefore always on a char
/// boundary. On an ASCII segment (the universal case) the character *is* the
/// byte and the step is a constant 1, so a read costs exactly what indexing a
/// pre-collected `Vec<char>` did; `seg.ascii` is loop-invariant, so the decode
/// branch unswitches rather than running per character.
fn glob_seg_match(toks: &[Tok], seg: &PathSeg<'_>) -> bool {
    let bytes = seg.text.as_bytes();
    let mut ti = 0;
    let mut si = 0;
    // (token index after the last `*`, byte offset where that `*` started)
    let mut star: Option<(usize, usize)> = None;
    while si < bytes.len() {
        if ti < toks.len() {
            let (c, width) = if seg.ascii {
                (char::from(bytes[si]), 1)
            } else {
                char_at(seg.text, si)
            };
            let matched = match &toks[ti] {
                Tok::Star => {
                    star = Some((ti + 1, si));
                    ti += 1;
                    continue;
                }
                Tok::Question => true,
                Tok::Lit(lit) => *lit == c,
                Tok::Class { negated, items } => class_matches(*negated, items, c),
            };
            if matched {
                ti += 1;
                si += width;
                continue;
            }
        }
        // mismatch (or tokens exhausted) — backtrack to the last `*` and let it
        // swallow one more character
        if let Some((sti, ssi)) = star {
            ti = sti;
            // `ssi` was recorded inside the loop, so it is a valid char boundary
            // below the segment's length — advancing by that char's width keeps
            // it one.
            let width = if seg.ascii {
                1
            } else {
                char_at(seg.text, ssi).1
            };
            si = ssi + width;
            star = Some((sti, si));
        } else {
            return false;
        }
    }
    // trailing `*`s can match the empty remainder
    while ti < toks.len() && matches!(toks[ti], Tok::Star) {
        ti += 1;
    }
    ti == toks.len()
}
