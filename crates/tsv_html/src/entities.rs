//! The named character references table — HTML's list of the names a `&…` reference may
//! spell, each with the characters it stands for — and the tokenizer's match against it.
//!
//! - Source: <https://html.spec.whatwg.org/entities.json>, one entry per name holding the
//!   characters it stands for
//! - Generated from `src/entities.json` into the committed `src/entities_table.rs` (a perfect
//!   hash table, see `entity`) by `deno task entities:update`; the generator runs as the
//!   `entities_table_is_fresh` test, which fails when the two disagree. The crate's `NOTICE`
//!   attributes the list and states its license
//! - Some names stand for **two** code points — a combining mark, a variation selector, or a
//!   second character — which is why a match answers a `&str`
//!
//! References:
//! - <https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references>
//! - <https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state>

use crate::entities_table::{ENTITIES, ENTITY_DISPLACEMENTS, ENTITY_SEED, LONGEST_ENTITY_NAME};

/// The longest named character reference at the start of `input`, or `None` when no name in
/// the table starts it.
///
/// `input` is the text **after** the `&` (the `&` itself is not part of any name). On a match
/// the answer is the count of `input` **bytes** the name consumed, then the characters the
/// reference stands for. The count is the name's length, which varies by more than the name:
/// a name with a `;` consumes it (`amp;` is 4), a legacy name without one does not (`amp` is
/// 3, whatever follows), so the caller reads the byte before the count to learn whether the
/// match ended on a `;`.
///
/// This is the HTML tokenizer's [named character reference state][state]: "consume the
/// maximum number of characters possible, where the consumed characters are one of the
/// identifiers in the [named character references][table] table". A name with a `;` and a
/// legacy one without it are both identifiers, so `amp;` matches whole and `amp` matches
/// where no `;` follows; `notit;` matches only `not`, leaving `it;`. What the caller does with
/// a match — the attribute-value rule for a legacy name, the parse errors — is the state's
/// next step, and the caller's.
///
/// Names are ASCII and matched exactly, so a match always ends on a character boundary of
/// `input`.
///
/// [state]: https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state
/// [table]: https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references
///
/// # Examples
///
/// ```
/// use tsv_html::match_named_character_reference;
///
/// assert_eq!(match_named_character_reference("amp;rest"), Some((4, "&")));
/// // A legacy name without its `;`
/// assert_eq!(match_named_character_reference("lt "), Some((2, "<")));
/// // The longest name that matches, not the whole run
/// assert_eq!(match_named_character_reference("notit;"), Some((3, "\u{ac}")));
/// // Two code points
/// assert_eq!(match_named_character_reference("NotEqualTilde;"), Some((14, "\u{2242}\u{338}")));
/// assert_eq!(match_named_character_reference("bogus;"), None);
/// ```
#[inline]
pub fn match_named_character_reference(input: &str) -> Option<(usize, &'static str)> {
    // Every name is an ASCII alphanumeric run with at most a trailing `;`, so a match lies
    // within the input's leading alphanumeric run (plus the `;` after it). The run ends at the
    // first byte that could not appear in a name — for a multibyte character its first byte —
    // so every slice below stays on a character boundary.
    let run_len = input
        .bytes()
        .position(|b| !b.is_ascii_alphanumeric())
        .unwrap_or(input.len());

    // A `;` can only close the whole run, and that name is the longest candidate: try it first
    // (the common case, `&amp;`).
    if input.as_bytes().get(run_len) == Some(&b';')
        && let Some(characters) = entity(&input[..=run_len])
    {
        return Some((run_len + 1, characters));
    }

    // Otherwise the longest prefix of the run that is a name. No name is longer than
    // `LONGEST_ENTITY_NAME`, so the search starts there rather than at the run's end — from the
    // end it would look up every prefix of the run, quadratic in a long alphanumeric run.
    (1..=run_len.min(LONGEST_ENTITY_NAME))
        .rev()
        .find_map(|len| entity(&input[..len]).map(|characters| (len, characters)))
}

/// The characters the named reference `name` stands for (`name` without its `&`, with its
/// semicolon if it has one), or `None` when no reference has that name.
///
/// [`ENTITIES`] is a perfect hash table, in the hash-and-displace shape (CHD) `phf` builds:
/// the hash picks a bucket, the bucket's displacement pair picks the one slot a name of that
/// hash can occupy, and the slot's name is compared with `name`. One hash and one compare,
/// whether or not `name` is a reference — [`match_named_character_reference`]'s prefix search asks for
/// up to [`LONGEST_ENTITY_NAME`] names that mostly are not. The generator
/// (`entities_table_is_fresh`) searches the seed and the displacements, through this same
/// [`entity_hash`] and [`entity_slot`]. Marked `#[inline]`, like the hash beneath it, so the
/// whole lookup compiles into a caller in another crate.
#[inline]
fn entity(name: &str) -> Option<&'static str> {
    let (name_slot, characters) = *ENTITIES.get(entity_slot(entity_hash(name.as_bytes())))?;
    (name_slot == name).then_some(characters)
}

/// The slot of [`ENTITIES`] a name with hash `hash` can occupy: phf's `get_index` over the
/// three fields [`entity_hash`] packs, a bucket and the two displacement coefficients.
#[inline]
pub(crate) fn entity_slot(hash: u64) -> usize {
    let (bucket, f1, f2) = entity_hash_parts(hash);
    let Some(&(d1, d2)) = ENTITY_DISPLACEMENTS.get(bucket as usize % ENTITY_DISPLACEMENTS.len())
    else {
        // unreachable: the index is reduced modulo the length
        return usize::MAX;
    };
    (d2.wrapping_add(f1.wrapping_mul(d1)).wrapping_add(f2) as usize) % ENTITIES.len()
}

/// The three fields of a name's hash: its bucket, then the two coefficients its bucket's
/// displacement pair is applied with.
#[inline]
pub(crate) const fn entity_hash_parts(hash: u64) -> (u32, u32, u32) {
    (
        (hash >> 43) as u32,
        ((hash >> 22) & 0x1F_FFFF) as u32,
        (hash & 0x3F_FFFF) as u32,
    )
}

/// The table's hash of a name: FNV-1a over its bytes from the generator's seed
/// ([`ENTITY_SEED`]), then splitmix64's finalizer, since FNV's last byte reaches only the
/// low bits and [`entity_hash_parts`] reads the high ones too.
#[inline]
pub(crate) fn entity_hash(name: &[u8]) -> u64 {
    entity_hash_seeded(name, ENTITY_SEED)
}

/// [`entity_hash`] from an explicit seed, which the generator varies.
#[inline]
pub(crate) fn entity_hash_seeded(name: &[u8], seed: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325 ^ seed;
    for &byte in name {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^ (hash >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated table's shape, which [`match_named_character_reference`] reads as a given: a name
    /// is an ASCII run with at most a trailing semicolon (its byte scan ends every name run on
    /// `is_ascii_alphanumeric`, and slices by byte), and a value is the one or two characters
    /// the reference stands for. A regeneration that dropped the second character would leave
    /// this the only guard — no corpus file in any gate uses a two-character reference.
    #[test]
    fn test_entity_table_shape() {
        let mut two_character = 0;
        for &(name, characters) in &ENTITIES {
            let bare = name.strip_suffix(';').unwrap_or(name);
            assert!(
                !bare.is_empty() && bare.bytes().all(|b| b.is_ascii_alphanumeric()),
                "{name:?} is not an ASCII name run plus an optional semicolon"
            );
            match characters.chars().count() {
                1 => {}
                2 => two_character += 1,
                n => panic!("{name:?} stands for {n} characters"),
            }
        }
        assert_eq!(ENTITIES.len(), 2231);
        assert_eq!(two_character, 93);
    }

    /// Every legacy name (no `;`) has a twin with one, standing for the same characters. So a
    /// run closed by `;` that is not itself a name is never a legacy name either, and a decoder
    /// built on [`match_named_character_reference`] cannot see the `;` of a whole-run legacy match.
    #[test]
    fn test_legacy_names_have_semicolon_twins() {
        for &(name, characters) in &ENTITIES {
            if !name.ends_with(';') {
                assert_eq!(entity(&format!("{name};")), Some(characters), "{name:?}");
            }
        }
    }

    /// [`entity`] finds every name in the table, each in its own slot, and nothing else.
    #[test]
    fn test_entity_table_lookup() {
        for (slot, &(name, characters)) in ENTITIES.iter().enumerate() {
            assert_eq!(entity_slot(entity_hash(name.as_bytes())), slot, "{name:?}");
            assert_eq!(entity(name), Some(characters), "{name:?}");
        }
        for absent in ["", "amp;;", "AMp;", "nbsp;x", "notin", "&amp;", "\u{e9}"] {
            assert_eq!(entity(absent), None, "{absent:?}");
        }
    }

    /// `LONGEST_ENTITY_NAME` bounds the name search, so it must be the table's longest key:
    /// shorter and a long name stops matching.
    #[test]
    fn test_longest_entity_name() {
        assert_eq!(
            ENTITIES.iter().map(|(name, _)| name.len()).max(),
            Some(LONGEST_ENTITY_NAME)
        );
        assert_eq!(
            match_named_character_reference("CounterClockwiseContourIntegral;"),
            Some((LONGEST_ENTITY_NAME, "\u{2233}"))
        );
    }

    /// Every name matches whole, as itself; with a `;` or alphanumeric tail after a legacy
    /// name, the longest match still wins.
    #[test]
    fn test_match_every_name() {
        for &(name, characters) in &ENTITIES {
            assert_eq!(
                match_named_character_reference(name),
                Some((name.len(), characters)),
                "{name:?}"
            );
            assert_eq!(
                match_named_character_reference(&format!("{name} tail")),
                Some((name.len(), characters)),
                "{name:?}"
            );
        }
        // A legacy name followed by more of the run: the longest name in the run wins.
        assert_eq!(
            match_named_character_reference("COPYRIGHT"),
            Some((4, "\u{a9}"))
        );
        // A `;` closing a run that is no name: the longest prefix that is one.
        assert_eq!(
            match_named_character_reference("notit;"),
            Some((3, "\u{ac}"))
        );
        assert_eq!(
            match_named_character_reference("notin;"),
            Some((6, "\u{2209}"))
        );
    }

    #[test]
    fn test_match_misses() {
        for input in ["", ";", "#65;", " amp;", "xyz;", "\u{e9}amp;", "AMp;"] {
            assert_eq!(match_named_character_reference(input), None, "{input:?}");
        }
        // A multibyte character ends the run on a boundary.
        assert_eq!(
            match_named_character_reference("amp\u{4e2d}"),
            Some((3, "&"))
        );
        assert_eq!(match_named_character_reference("a\u{4e2d}pos;"), None);
    }

    /// A long alphanumeric run names no reference; the search is bounded by the longest name,
    /// not the run.
    #[test]
    fn test_long_name_run() {
        assert_eq!(match_named_character_reference(&"a".repeat(10_000)), None);
        assert_eq!(
            match_named_character_reference(&format!("amp{}", "a".repeat(10_000))),
            Some((3, "&"))
        );
    }
}
