//! ASCII-case-insensitive keyword sets with a shape pre-filter in front of the hash.
//!
//! CSS is full of fixed vocabularies — named colors, units — that a value is checked
//! against once per token. A `phf` probe answers in O(1), but O(1) is not free: the hash
//! walks every byte, and on these paths the answer is almost always *no* (the value parser
//! asks "is this a color?" of every `var`, `auto`, `solid` and `--custom-property` it
//! builds). Every keyword in these sets is a short run of pure lowercase ASCII letters, so
//! a few instructions of shape checking retire the overwhelming majority before the hash
//! runs at all.
//!
//! [`ascii_keyword_set!`] is the way to declare one. The literal list is written **once**
//! and drives everything: the `phf` set the lookup probes, the length bounds and
//! first-letter→length bitmap the pre-filter rejects on, a compile-time assertion that
//! every keyword really is pure lowercase ASCII letters — the invariant the pre-filter
//! rests on — and an exhaustive test grading the whole lookup against a plain scan of the
//! same list. A keyword that broke the invariant (a digit, a hyphen, a 32nd byte) fails
//! the build rather than silently blinding the filter, which is the failure mode to fear:
//! a table and the set it guards, written by hand as two copies, drift apart without a
//! single test going red.
//!
//! The bundled test is not a nicety. The classifications these sets drive are **not
//! observable in formatted output today** (a `Color::Named` prints exactly like the
//! `Identifier` it would otherwise have been — see `tsv_css/CLAUDE.md`), so no corpus diff
//! can grade them; a lookup that wrongly rejected `red` would pass every gate this repo
//! has. When the linter and LSP land, that changes — those tools read the classification
//! directly and a wrong answer becomes a wrong diagnostic. Until then the generated test
//! is the only thing standing under these sets, so it ships with every one of them.

/// A keyword set's byte-shape, derived from its members by [`Shape::of`].
///
/// Sound by construction: the pre-filter rejects only strings that *cannot* be members
/// (wrong length, a non-letter, or no member of that length starts with that letter), so
/// gating a probe on it can never change an answer — only skip work.
pub(crate) struct Shape {
    min_len: usize,
    max_len: usize,
    /// Bit `n` of `lengths_by_first_letter[c - b'a']` is set iff some keyword starts with
    /// `c` and is exactly `n` bytes long. One load rejects `var` (no 3-letter `v` color),
    /// `solid` and `flex` — and, via the letter check below, every `--custom-property`.
    lengths_by_first_letter: [u32; 26],
}

/// Bounds both the `u32` length bitmap and the stack buffer the case-folding path uses.
const MAX_KEYWORD_LEN: usize = 32;

impl Shape {
    /// Derive the shape of `keywords`, asserting the invariant the pre-filter depends on.
    pub(crate) const fn of(keywords: &[&str]) -> Self {
        let (mut min_len, mut max_len) = (usize::MAX, 0);
        let mut lengths_by_first_letter = [0u32; 26];

        let mut i = 0;
        while i < keywords.len() {
            let keyword = keywords[i].as_bytes();
            let len = keyword.len();
            assert!(
                len < MAX_KEYWORD_LEN,
                "keyword is too long for the length bitmap and the case-folding buffer"
            );
            // An empty keyword would drag `min_len` to 0, and `probe` reads `s[0]` as soon
            // as the length check passes — so it would turn `probe("")` into a panic.
            assert!(!keyword.is_empty(), "a keyword must not be empty");

            let mut j = 0;
            while j < len {
                assert!(
                    keyword[j].is_ascii_lowercase(),
                    "a keyword must be pure lowercase ASCII letters — the pre-filter \
                     rejects anything else without ever probing the set"
                );
                j += 1;
            }

            if len < min_len {
                min_len = len;
            }
            if len > max_len {
                max_len = len;
            }
            lengths_by_first_letter[(keyword[0] - b'a') as usize] |= 1 << len;
            i += 1;
        }

        Self {
            min_len,
            max_len,
            lengths_by_first_letter,
        }
    }
}

/// Is `s` a member of `set`, compared ASCII-case-insensitively?
///
/// Allocation-free either way: an already-lowercase `s` (the common case) probes the set by
/// reference, and a mixed-case one folds into a stack buffer rather than a `String`. Reach
/// for [`ascii_keyword_set!`] rather than calling this directly — the macro is what pairs a
/// set with the `Shape` derived from that same set.
#[inline]
pub(crate) fn probe(s: &str, shape: &Shape, set: &phf::Set<&'static str>) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();

    if !(shape.min_len..=shape.max_len).contains(&len) {
        return false;
    }
    let first = bytes[0].to_ascii_lowercase();
    if !first.is_ascii_lowercase()
        || shape.lengths_by_first_letter[(first - b'a') as usize] & (1 << len) == 0
    {
        return false;
    }

    // One pass settles both remaining questions: a non-letter anywhere is a reject (a
    // length-plausible survivor can still hold one — `sans-serif` is a 10-byte `s` word,
    // and so is `sandybrown`), and uppercase means the probe must fold the input first.
    let mut has_uppercase = false;
    for &byte in bytes {
        if !byte.is_ascii_alphabetic() {
            return false;
        }
        has_uppercase |= byte.is_ascii_uppercase();
    }

    if !has_uppercase {
        return set.contains(s);
    }

    let mut folded = [0u8; MAX_KEYWORD_LEN];
    let folded = &mut folded[..len];
    for (dst, &byte) in folded.iter_mut().zip(bytes) {
        *dst = byte.to_ascii_lowercase();
    }
    // ASCII letters by the loop above, so the validation always succeeds.
    str::from_utf8(folded).is_ok_and(|folded| set.contains(folded))
}

/// Declare an ASCII-case-insensitive keyword set, its membership test, and the exhaustive
/// test that grades one against the other.
///
/// The keyword list is the single source of truth for the `phf` set *and* the pre-filter's
/// tables — see the module docs for why that matters. Expands to a private `static` set, a
/// membership function whose tables are `const` (so they fold into immediates), and a
/// `#[cfg(test)]` module carrying the equivalence proof.
macro_rules! ascii_keyword_set {
    (
        $(#[$set_meta:meta])* static $set:ident;
        $(#[$fn_meta:meta])* $vis:vis fn $probe:ident;
        $($keyword:literal),* $(,)?
    ) => {
        $(#[$set_meta])*
        static $set: phf::Set<&'static str> = phf::phf_set! { $($keyword),* };

        $(#[$fn_meta])*
        $vis fn $probe(s: &str) -> bool {
            const KEYWORDS: &[&str] = &[$($keyword),*];
            const SHAPE: $crate::keyword_set::Shape = $crate::keyword_set::Shape::of(KEYWORDS);
            $crate::keyword_set::probe(s, &SHAPE, &$set)
        }

        #[cfg(test)]
        mod keyword_set_tests {
            /// The lookup must agree with a plain scan of the same keyword list on every
            /// input — the pre-filter may only skip work, never change an answer.
            #[test]
            fn agrees_with_a_plain_scan_of_the_keyword_list() {
                $crate::keyword_set::test_support::verify(
                    super::$probe,
                    &[$($keyword),*],
                    stringify!($set),
                );
            }
        }
    };
}

pub(crate) use ascii_keyword_set;

#[cfg(test)]
pub(crate) mod test_support {
    /// Grade `probe` against the obvious, slow, obviously-correct implementation:
    /// a linear ASCII-case-insensitive scan of the very list the set was built from.
    ///
    /// Exhaustive where it counts. Every keyword in three casings, plus the near-misses a
    /// shape filter is most likely to get wrong (a keyword with a letter added, removed, or
    /// a hyphen glued on); every string of length 0..=3 over the bytes CSS identifiers are
    /// made of, which totally covers both sets' short members (`tan`, `red`, `px`, `q`) and
    /// every bitmap cell such an input can reach; and letter runs past both length bounds.
    pub(crate) fn verify(probe: fn(&str) -> bool, keywords: &[&str], label: &str) {
        let reference = |s: &str| keywords.iter().any(|kw| kw.eq_ignore_ascii_case(s));

        let check = |s: &str| {
            assert_eq!(
                probe(s),
                reference(s),
                "{label}: the pre-filtered lookup disagreed with a plain scan on {s:?}"
            );
        };

        for keyword in keywords {
            assert!(probe(keyword), "{label}: {keyword} stopped being a member");
            assert!(probe(&keyword.to_ascii_uppercase()));
            let mut mixed = (*keyword).to_string();
            mixed[..1].make_ascii_uppercase();
            assert!(probe(&mixed));

            check(&format!("{keyword}x"));
            check(&keyword[1..]);
            check(&format!("-{keyword}"));
            check(&format!("{keyword}-"));
        }

        // Real CSS text: the leaves a value parser builds and the units a printer sees.
        for s in [
            "",
            "var",
            "auto",
            "none",
            "solid",
            "flex",
            "block",
            "grid",
            "center",
            "space-between",
            "sans-serif",
            "monospace",
            "bold",
            "normal",
            "border-box",
            "currentColor",
            "1px",
            "0.5rem",
            "100%",
            "--color-primary",
            "-webkit-box",
            "u",
            "to",
            "9",
            "#fff",
            "rgb",
            "calc",
            "px",
            "PX",
            "Px",
            "rem",
            "vmin",
            "q",
            "Q",
            "hz",
            "kHz",
            "dppx",
            "fr",
            "s",
            "ms",
            "foo",
            "n",
            "café",
            "réd",
            "ταν",
        ] {
            check(s);
        }

        let alphabet: Vec<u8> = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .chain([b'-', b'_'])
            .collect();
        let mut buf = String::with_capacity(3);
        for &a in &alphabet {
            buf.clear();
            buf.push(a as char);
            check(&buf);
            for &b in &alphabet {
                buf.truncate(1);
                buf.push(b as char);
                check(&buf);
                for &c in &alphabet {
                    buf.truncate(2);
                    buf.push(c as char);
                    check(&buf);
                }
            }
        }

        // Letter runs either side of both length bounds — a run one byte longer than the
        // longest keyword can never be a member, whatever it spells.
        let longest = keywords.iter().map(|kw| kw.len()).max().unwrap_or(0);
        for len in 0..=(longest + 2) {
            for lead in ["a", "l", "p", "z"] {
                check(&(lead.to_string() + &"a".repeat(len.saturating_sub(1))));
            }
        }
    }
}
