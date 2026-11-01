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
//!   second character — which is why a match's characters are a `&str`
//!
//! References:
//! - <https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references>
//! - <https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state>

use crate::entities_table::{ENTITIES, ENTITY_DISPLACEMENTS, ENTITY_SEED, LONGEST_ENTITY_NAME};

/// A named character reference [`match_named_character_reference`] found: a row of the
/// spec's [named character references][table] table, its **name** and the **characters** it
/// stands for.
///
/// Only a match makes one; both halves are the table's own `'static` strings, so it borrows
/// nothing from the input it was matched in.
///
/// [table]: https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedCharacterReference {
    name: &'static str,
    characters: &'static str,
}

impl NamedCharacterReference {
    /// The matched name as the table spells it, without the `&` and with its `;` when the
    /// match took one (`"amp;"`, or the legacy `"amp"` where no `;` follows).
    ///
    /// Names are ASCII and matched exactly, so the name is byte-equal to the start of the
    /// input: `name().len()` is the count of input bytes the match consumed (always on a
    /// character boundary), and `name().ends_with(';')` says whether the match ended on a
    /// `;` — the question the tokenizer's next step asks.
    #[inline]
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The one or two characters the reference stands for (`"&"` for `amp;`,
    /// `"\u{2242}\u{338}"` for `NotEqualTilde;`).
    #[inline]
    #[must_use]
    pub const fn characters(self) -> &'static str {
        self.characters
    }
}

/// The longest named character reference at the start of `input`, or `None` when no name in
/// the table starts it.
///
/// `input` is the text **after** the `&` (the `&` itself is not part of any name). A match
/// answers the name it consumed and the characters that name stands for
/// ([`NamedCharacterReference`]). One reference can match with or without its `;`: `amp;`
/// takes 4 bytes, the legacy `amp` takes 3 whatever follows, so
/// [`NamedCharacterReference::name`] carries both how much of `input` the match took and
/// whether it ended on a `;`.
///
/// This is the HTML tokenizer's [named character reference state][state]: "consume the
/// maximum number of characters possible, where the consumed characters are one of the
/// identifiers in the [named character references][table] table". A name with a `;` and a
/// legacy one without it are both identifiers, so `amp;` matches whole and `amp` matches
/// where no `;` follows; `notit;` matches only `not`, leaving `it;`. What the caller does with
/// a match — the attribute-value rule for a legacy name, the parse errors — is the state's
/// next step, and the caller's.
///
/// [state]: https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state
/// [table]: https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references
///
/// # Examples
///
/// ```
/// use tsv_html::match_named_character_reference;
///
/// let amp = match_named_character_reference("amp;rest").unwrap();
/// assert_eq!((amp.name(), amp.characters()), ("amp;", "&"));
/// // How much of the input the match consumed, and whether a `;` closed it
/// assert_eq!(amp.name().len(), 4);
/// assert!(amp.name().ends_with(';'));
///
/// let matched = |input: &str| {
///     match_named_character_reference(input).map(|r| (r.name(), r.characters()))
/// };
/// // A legacy name without its `;`
/// assert_eq!(matched("lt "), Some(("lt", "<")));
/// // The longest name that matches, not the whole run
/// assert_eq!(matched("notit;"), Some(("not", "\u{ac}")));
/// // Two code points
/// assert_eq!(matched("NotEqualTilde;"), Some(("NotEqualTilde;", "\u{2242}\u{338}")));
/// assert_eq!(matched("bogus;"), None);
/// ```
#[inline]
#[must_use]
pub fn match_named_character_reference(input: &str) -> Option<NamedCharacterReference> {
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
        && let Some(reference) = entity(&input[..=run_len])
    {
        return Some(reference);
    }

    // Otherwise the longest prefix of the run that is a name. No name is longer than
    // `LONGEST_ENTITY_NAME`, so the search starts there rather than at the run's end — from the
    // end it would look up every prefix of the run, quadratic in a long alphanumeric run.
    (1..=run_len.min(LONGEST_ENTITY_NAME))
        .rev()
        .find_map(|len| entity(&input[..len]))
}

/// The table's row for the named reference `name` (`name` without its `&`, with its
/// semicolon if it has one), or `None` when no reference has that name. The row's name is the
/// table's `'static` copy, byte-equal to `name`.
///
/// [`ENTITIES`] is a perfect hash table, in the hash-and-displace shape (CHD) `phf` builds:
/// the hash picks a bucket, the bucket's displacement pair picks the one slot a name of that
/// hash can occupy, and the slot's name is compared with `name`. One hash and one compare,
/// whether or not `name` is a reference — [`match_named_character_reference`]'s prefix search
/// asks for up to [`LONGEST_ENTITY_NAME`] names that mostly are not. The generator
/// (`entities_table_is_fresh`) searches the seed and the displacements, through this same
/// [`entity_hash`] and [`entity_slot`]. Marked `#[inline]`, like the hash beneath it, so the
/// whole lookup compiles into a caller in another crate.
#[inline]
fn entity(name: &str) -> Option<NamedCharacterReference> {
    let (name_slot, characters) = *ENTITIES.get(entity_slot(entity_hash(name.as_bytes())))?;
    (name_slot == name).then_some(NamedCharacterReference {
        name: name_slot,
        characters,
    })
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

    /// The match a test expects: `name` as the table spells it, standing for `characters`.
    fn reference(name: &'static str, characters: &'static str) -> NamedCharacterReference {
        NamedCharacterReference { name, characters }
    }

    /// The generated table's shape, which [`match_named_character_reference`] reads as a given:
    /// a name is an ASCII run with at most a trailing semicolon (its byte scan ends every name
    /// run on `is_ascii_alphanumeric`, and slices by byte), and a value is the one or two
    /// characters the reference stands for. A regeneration that dropped the second character
    /// would leave this the only guard — no corpus file in any gate uses a two-character
    /// reference.
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
    /// built on [`match_named_character_reference`] cannot see the `;` of a whole-run legacy
    /// match.
    #[test]
    fn test_legacy_names_have_semicolon_twins() {
        for &(name, characters) in &ENTITIES {
            if !name.ends_with(';') {
                assert_eq!(
                    entity(&format!("{name};")).map(NamedCharacterReference::characters),
                    Some(characters),
                    "{name:?}"
                );
            }
        }
    }

    /// [`entity`] finds every name in the table, each in its own slot, and nothing else.
    #[test]
    fn test_entity_table_lookup() {
        for (slot, &(name, characters)) in ENTITIES.iter().enumerate() {
            assert_eq!(entity_slot(entity_hash(name.as_bytes())), slot, "{name:?}");
            assert_eq!(entity(name), Some(reference(name, characters)), "{name:?}");
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
        let longest = match_named_character_reference("CounterClockwiseContourIntegral;");
        assert_eq!(
            longest,
            Some(reference("CounterClockwiseContourIntegral;", "\u{2233}"))
        );
        assert_eq!(longest.map(|r| r.name().len()), Some(LONGEST_ENTITY_NAME));
    }

    /// Every name matches whole, as itself; with a `;` or alphanumeric tail after a legacy
    /// name, the longest match still wins.
    #[test]
    fn test_match_every_name() {
        for &(name, characters) in &ENTITIES {
            assert_eq!(
                match_named_character_reference(name),
                Some(reference(name, characters)),
                "{name:?}"
            );
            assert_eq!(
                match_named_character_reference(&format!("{name} tail")),
                Some(reference(name, characters)),
                "{name:?}"
            );
        }
        // A legacy name followed by more of the run: the longest name in the run wins.
        assert_eq!(
            match_named_character_reference("COPYRIGHT"),
            Some(reference("COPY", "\u{a9}"))
        );
        // A `;` closing a run that is no name: the longest prefix that is one.
        assert_eq!(
            match_named_character_reference("notit;"),
            Some(reference("not", "\u{ac}"))
        );
        assert_eq!(
            match_named_character_reference("notin;"),
            Some(reference("notin;", "\u{2209}"))
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
            Some(reference("amp", "&"))
        );
        assert_eq!(match_named_character_reference("a\u{4e2d}pos;"), None);
    }

    /// The name a match answers is the start of `input`, byte for byte: what the caller skips.
    #[test]
    fn test_name_is_the_consumed_prefix() {
        for input in [
            "amp;",
            "amp",
            "amp tail",
            "amp\u{4e2d}",
            "ampamp;",
            "COPYRIGHT",
            "notit;",
            "notin;x",
            "\u{e9}",
        ] {
            if let Some(matched) = match_named_character_reference(input) {
                assert!(input.starts_with(matched.name()), "{input:?}");
            }
        }
    }

    /// A long alphanumeric run names no reference; the search is bounded by the longest name,
    /// not the run.
    #[test]
    fn test_long_name_run() {
        assert_eq!(match_named_character_reference(&"a".repeat(10_000)), None);
        assert_eq!(
            match_named_character_reference(&format!("amp{}", "a".repeat(10_000))),
            Some(reference("amp", "&"))
        );
    }
}
