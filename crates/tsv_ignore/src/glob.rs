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

/// A path segment paired with its chars, collected once. `match_segments` runs
/// the same path against many rules across many ancestor prefixes; collecting
/// each segment's chars here (in [`path_segments`](super::path_segments), once
/// per path) keeps the inner glob match from re-collecting them per rule. `text`
/// powers the cheap `&str` anchor comparison in `Layer::relativize`.
#[derive(Debug)]
pub(crate) struct PathSeg<'a> {
    pub(crate) text: &'a str,
    chars: Vec<char>,
}

impl<'a> PathSeg<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self {
            text,
            chars: text.chars().collect(),
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

/// Matches a pattern's segments against a path's segments, with `**` consuming
/// zero or more path segments.
pub(crate) fn match_segments(pat: &[Seg], path: &[PathSeg<'_>]) -> bool {
    match pat.split_first() {
        None => path.is_empty(),
        Some((Seg::DoubleStar, rest)) => {
            // `**` matches any number of leading path segments. A *trailing*
            // `**` must consume at least one: git's `foo/**` matches everything
            // *inside* foo, never foo itself (so a later `!foo/keep.ts` can
            // re-include the file). An interior `**` still matches zero, so
            // `a/**/b` keeps matching `a/b`.
            let start = usize::from(rest.is_empty());
            (start..=path.len()).any(|i| match_segments(rest, &path[i..]))
        }
        Some((Seg::Glob(toks), rest)) => match path.split_first() {
            Some((head, tail)) if glob_seg_match(toks, &head.chars) => match_segments(rest, tail),
            _ => false,
        },
    }
}

/// Matches one path segment against a segment's tokens. Classic two-pointer
/// glob match with single-star backtracking; segments are short so this is
/// cheap.
fn glob_seg_match(toks: &[Tok], seg: &[char]) -> bool {
    let mut ti = 0;
    let mut si = 0;
    // (token index after the last `*`, path index where that `*` started)
    let mut star: Option<(usize, usize)> = None;
    while si < seg.len() {
        if ti < toks.len() {
            let matched = match &toks[ti] {
                Tok::Star => {
                    star = Some((ti + 1, si));
                    ti += 1;
                    continue;
                }
                Tok::Question => true,
                Tok::Lit(c) => *c == seg[si],
                Tok::Class { negated, items } => class_matches(*negated, items, seg[si]),
            };
            if matched {
                ti += 1;
                si += 1;
                continue;
            }
        }
        // mismatch (or tokens exhausted) — backtrack to the last `*` and let it
        // swallow one more character
        if let Some((sti, ssi)) = star {
            ti = sti;
            si = ssi + 1;
            star = Some((sti, ssi + 1));
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
