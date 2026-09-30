//! ASCII-case-insensitive keyword sets, answered from a table the keyword list builds at
//! compile time.
//!
//! CSS is full of fixed vocabularies — named colors, units — that a value is checked
//! against once per token, and on these paths the answer is almost always *no* (the value
//! parser asks "is this a color?" of every `var`, `auto`, `solid` and `--custom-property`
//! it builds). Every keyword in these sets is a short run of pure lowercase ASCII letters,
//! so a few instructions of shape checking retire the overwhelming majority before any
//! member is compared at all.
//!
//! The lookup is a **bucket table keyed on the first letter and the length**, not a hash
//! and not a `match`. A `phf` probe is O(1) but not cheap: it reads every byte through
//! SipHash, divides twice to find its slot, and only then compares — through a `bcmp`
//! call — the one candidate it landed on. A `match` over the string literals compiles to a
//! switch on the length and then a linear chain of compares against every member of that
//! length, which is fine for a handful of members and is not for the named colors: `none`
//! is compared against every four-letter color before it is refused. The bucket a probe
//! lands in holds only the members sharing its first letter and its length — one or two,
//! often none — and each member is stored as the little-endian words of its bytes, so a
//! candidate is compared as a few integer compares against the input folded into the same
//! words.
//!
//! [`ascii_keyword_set!`] is the way to declare one. The literal list is written **once**
//! and drives everything: the length bounds and the bucket table the lookup reads, a
//! compile-time assertion that every keyword really is pure lowercase ASCII letters — the
//! invariant the case fold and the table rest on — and an exhaustive test grading the whole
//! lookup against a plain scan of the same list. A keyword that broke the invariant (a
//! digit, a hyphen, a 32nd byte) fails the build rather than silently blinding the lookup,
//! which is the failure mode to fear: a table and the set it encodes, written by hand as
//! two copies, drift apart without a single test going red.
//!
//! The bundled test is not a nicety. The classifications these sets drive are **not
//! observable in formatted output today** (a `Color::Named` prints exactly like the
//! `Identifier` it would otherwise have been — see `tsv_css/CLAUDE.md`), so no corpus diff
//! can grade them; a lookup that wrongly rejected `red` would pass every gate this repo
//! has. When the linter and LSP land, that changes — those tools read the classification
//! directly and a wrong answer becomes a wrong diagnostic. Until then the generated test
//! is the only thing standing under these sets, so it ships with every one of them.

/// Bounds a keyword's length: the bucket table's per-letter row width and the stack buffer
/// the case fold writes into.
const MAX_KEYWORD_LEN: usize = 32;

/// One bucket per first letter and length.
const BUCKETS: usize = 26 * MAX_KEYWORD_LEN;

/// The bucket a keyword starting with the lowercase letter `first` and `len` bytes long
/// lives in.
const fn bucket_of(first: u8, len: usize) -> usize {
    (first - b'a') as usize * MAX_KEYWORD_LEN + len
}

/// How many `u64` words the longest of `keywords` spans — the `W` its [`Table`] stores
/// each member in.
pub(crate) const fn words_for(keywords: &[&str]) -> usize {
    let mut max_len = 0;
    let mut i = 0;
    while i < keywords.len() {
        if keywords[i].len() > max_len {
            max_len = keywords[i].len();
        }
        i += 1;
    }
    max_len.div_ceil(8)
}

/// A keyword set's lookup table, derived from its `N` members by [`Table::of`]: the length
/// bounds, and every member stored as `W` little-endian words, sorted into buckets by first
/// letter and length.
///
/// Sound by construction: a string is a member exactly when its fold lands in a bucket
/// holding a member with the same words, and the length and letter checks in front of the
/// bucket only refuse strings no bucket could hold — so they can skip work but never change
/// an answer.
pub(crate) struct Table<const N: usize, const W: usize> {
    min_len: usize,
    max_len: usize,
    /// The members of bucket `b` are `words[starts[b]..starts[b + 1]]`.
    starts: [u8; BUCKETS + 1],
    /// Every member's bytes as little-endian words, zero past its end, in bucket order.
    words: [[u64; W]; N],
}

impl<const N: usize, const W: usize> Table<N, W> {
    /// Build the table for `keywords`, asserting the invariant the lookup depends on.
    pub(crate) const fn of(keywords: &[&str]) -> Self {
        assert!(
            keywords.len() == N,
            "the table is sized for a different keyword count"
        );
        assert!(N <= u8::MAX as usize, "bucket starts are stored as `u8`");
        assert!(
            W * 8 <= MAX_KEYWORD_LEN,
            "a member's words outrun the fold buffer"
        );

        let (mut min_len, mut max_len) = (usize::MAX, 0);
        // Counting sort into buckets: count each bucket's members one slot ahead of it…
        let mut starts = [0u8; BUCKETS + 1];
        let mut i = 0;
        while i < N {
            let keyword = keywords[i].as_bytes();
            let len = keyword.len();
            assert!(
                len < MAX_KEYWORD_LEN && len <= W * 8,
                "keyword is too long for the bucket table and the case-folding buffer"
            );
            // An empty keyword has no first letter to bucket it by.
            assert!(!keyword.is_empty(), "a keyword must not be empty");
            let mut j = 0;
            while j < len {
                assert!(
                    keyword[j].is_ascii_lowercase(),
                    "a keyword must be pure lowercase ASCII letters — the lookup folds its \
                     input to lowercase and refuses anything else without comparing"
                );
                j += 1;
            }
            if len < min_len {
                min_len = len;
            }
            if len > max_len {
                max_len = len;
            }
            starts[bucket_of(keyword[0], len) + 1] += 1;
            i += 1;
        }
        // …turn the counts into each bucket's first slot…
        let mut b = 0;
        while b < BUCKETS {
            starts[b + 1] += starts[b];
            b += 1;
        }
        // …and place each member at its bucket's next free slot.
        let mut next = starts;
        let mut words = [[0u64; W]; N];
        let mut i = 0;
        while i < N {
            let keyword = keywords[i].as_bytes();
            let bucket = bucket_of(keyword[0], keyword.len());
            let slot = next[bucket] as usize;
            next[bucket] += 1;
            let mut j = 0;
            while j < keyword.len() {
                words[slot][j / 8] |= (keyword[j] as u64) << (8 * (j % 8));
                j += 1;
            }
            i += 1;
        }

        Self {
            min_len,
            max_len,
            starts,
            words,
        }
    }
}

/// Is `s` a member of `table`'s set, compared ASCII-case-insensitively?
///
/// Allocation-free: the input is folded to lowercase into a stack buffer as it is checked
/// for letters, and compared a word at a time against the members of its bucket. Reach for
/// [`ascii_keyword_set!`] rather than calling this directly — the macro is what builds the
/// table from the set's own list.
#[inline]
pub(crate) fn probe<const N: usize, const W: usize>(s: &str, table: &Table<N, W>) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();

    if !(table.min_len..=table.max_len).contains(&len) {
        return false;
    }
    let first = bytes[0].to_ascii_lowercase();
    if !first.is_ascii_lowercase() {
        return false;
    }
    let bucket = bucket_of(first, len);
    let (lo, hi) = (
        table.starts[bucket] as usize,
        table.starts[bucket + 1] as usize,
    );
    // No member starts with this letter and has this length: one load rejects `var` (no
    // 3-letter `v` color), `solid` and `flex`.
    if lo == hi {
        return false;
    }

    // One pass refuses a non-letter anywhere (a bucket's survivor can still hold one —
    // `sans-serif` is a 10-byte `s` word, and so is `sandybrown`) and folds the rest: an
    // ASCII letter's lowercase is the letter with bit 5 set.
    let mut folded = [0u8; MAX_KEYWORD_LEN];
    for (dst, &byte) in folded.iter_mut().zip(bytes) {
        if !byte.is_ascii_alphabetic() {
            return false;
        }
        *dst = byte | 0x20;
    }
    let (chunks, _) = folded.as_chunks::<8>();
    let mut input = [0u64; W];
    for (word, chunk) in input.iter_mut().zip(chunks) {
        *word = u64::from_le_bytes(*chunk);
    }
    table.words[lo..hi].contains(&input)
}

/// Declare an ASCII-case-insensitive keyword set, its membership test, and the exhaustive
/// test that grades one against the other.
///
/// The keyword list is the single source of truth for the lookup table — see the module
/// docs for why that matters. Expands to the list itself (`$set`, a `const` slice carrying
/// the set's doc comment — declared `static` in the invocation, which names what it is
/// rather than how it is stored), a membership function reading a `const` table built from
/// it, and a `#[cfg(test)]` module carrying the equivalence proof.
macro_rules! ascii_keyword_set {
    (
        $(#[$set_meta:meta])* static $set:ident;
        $(#[$fn_meta:meta])* $vis:vis fn $probe:ident;
        $($keyword:literal),* $(,)?
    ) => {
        $(#[$set_meta])*
        const $set: &[&str] = &[$($keyword),*];

        $(#[$fn_meta])*
        $vis fn $probe(s: &str) -> bool {
            const N: usize = $set.len();
            const W: usize = $crate::keyword_set::words_for($set);
            static TABLE: $crate::keyword_set::Table<N, W> = $crate::keyword_set::Table::of($set);
            $crate::keyword_set::probe(s, &TABLE)
        }

        #[cfg(test)]
        mod keyword_set_tests {
            /// The lookup must agree with a plain scan of the same keyword list on every
            /// input — the table lookup may only skip work, never change an answer.
            #[test]
            fn agrees_with_a_plain_scan_of_the_keyword_list() {
                $crate::keyword_set::test_support::verify(
                    super::$probe,
                    super::$set,
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
    /// made of, which totally covers every set's short members (`tan`, `red`, `px`, `q`) and
    /// every bucket such an input can reach; and letter runs past both length bounds.
    pub(crate) fn verify(probe: fn(&str) -> bool, keywords: &[&str], label: &str) {
        let reference = |s: &str| keywords.iter().any(|kw| kw.eq_ignore_ascii_case(s));

        let check = |s: &str| {
            assert_eq!(
                probe(s),
                reference(s),
                "{label}: the table lookup disagreed with a plain scan on {s:?}"
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
            // Same length, one byte off: the input lands in the keyword's own bucket, so
            // only a full-word compare can refuse it.
            for i in 0..keyword.len() {
                let mut near = keyword.as_bytes().to_vec();
                near[i] = if near[i] == b'z' { b'a' } else { near[i] + 1 };
                check(std::str::from_utf8(&near).unwrap());
            }
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
            // The bytes an ASCII `| 0x20` fold would carry onto a letter or a letter onto.
            .chain([b'@', b'[', b'`', b'{', b'^', b'~', 0x7f])
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
