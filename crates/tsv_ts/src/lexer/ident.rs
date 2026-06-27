// ECMAScript identifier grammar.
//
// Single source of truth for "is this char a valid identifier start/continue?"
// for the lexer (tokenizing identifiers), keyed on the Unicode `ID_Start`/
// `ID_Continue` properties ECMAScript uses. The printer's object-key unquoting is
// a *separate* question (does prettier strip the quotes?) and deliberately does
// NOT share these helpers — it stays on the narrower `XID` subset; see
// `printer::expressions::literals::is_valid_js_identifier`. That keeps the two
// independent and is sound for idempotency (`XID ⊆ ID`, so any key the printer
// unquotes still re-lexes here).

use unicode_ident::{is_xid_continue, is_xid_start};

/// `IdentifierStart`: the Unicode `ID_Start` property, plus the ECMAScript `_`
/// and `$` allowances.
///
/// ECMAScript identifiers use `ID_Start`/`ID_Continue` (ecma262
/// §sec-names-and-keywords → UAX #31), **not** the NFKC-closed `XID_Start`/
/// `XID_Continue` that `unicode_ident` reports. `ID_Start` is a superset, so the
/// `XID_Start` check is widened by `is_id_start_not_xid`. `_` is in
/// `XID_Continue` but not `XID_Start`, so it's checked explicitly.
#[inline]
pub(crate) fn is_id_start(ch: char) -> bool {
    is_xid_start(ch) || ch == '_' || ch == '$' || is_id_start_not_xid(ch)
}

/// `IdentifierPart`: the Unicode `ID_Continue` property, plus `$` (`_` and
/// ZWNJ/ZWJ are already in `ID_Continue`).
///
/// `ID_Continue \ XID_Continue` is a subset of `ID_Start \ XID_Start`, so reusing
/// `is_id_start_not_xid` here is exact: the four code points it adds that *are*
/// already in `XID_Continue` (U+0E33, U+0EB3, U+FF9E, U+FF9F) just short-circuit
/// on the `is_xid_continue` check.
#[inline]
pub(crate) fn is_id_continue(ch: char) -> bool {
    is_xid_continue(ch) || ch == '$' || is_id_start_not_xid(ch)
}

/// The code points in Unicode `ID_Start` but absent from `XID_Start` — the gap
/// between the property ECMAScript actually uses and the NFKC-closed variant
/// `unicode_ident` exposes. Two causes, all frozen:
///
/// - the `Other_ID_Start` voiced/semi-voiced sound marks (U+309B, U+309C), which
///   NFKC-decompose and so are dropped from `XID_Start` (the other
///   `Other_ID_Start` code points — U+1885/U+1886/U+2118/U+212E — *are* in
///   `XID_Start`, so they need no special-casing);
/// - letters whose NFKC decomposition leaves the identifier set: U+037A (Greek
///   ypogegrammeni), U+0E33 (Thai sara am), U+0EB3 (Lao am), U+FF9E/U+FF9F
///   (halfwidth katakana sound marks), and the Arabic ligature/presentation
///   forms U+FC5E–U+FC63, U+FDFA/U+FDFB, and U+FE70/72/74/76/78/7A/7C/7E.
///
/// This is `ID_Start \ XID_Start` computed from the UAX #31 property definition
/// (general category ∪ `Other_ID_Start` − `Pattern_Syntax` − `Pattern_White_Space`)
/// against `unicode_ident`'s `XID_Start`, cross-checked against acorn. The set is
/// stable across Unicode versions (NFKC decompositions and `Pattern_Syntax` are
/// immutable by Unicode's stability policy); revisit only on a major UCD bump that
/// adds a new NFKC-decomposing letter.
#[inline]
fn is_id_start_not_xid(ch: char) -> bool {
    matches!(
        ch,
        '\u{037A}'                  // GREEK YPOGEGRAMMENI
        | '\u{0E33}'                // THAI CHARACTER SARA AM
        | '\u{0EB3}'                // LAO VOWEL SIGN AM
        | '\u{309B}'..='\u{309C}'   // KATAKANA-HIRAGANA (SEMI-)VOICED SOUND MARK
        | '\u{FC5E}'..='\u{FC63}'   // ARABIC LIGATURE … WITH … ISOLATED FORM
        | '\u{FDFA}'..='\u{FDFB}'   // ARABIC LIGATURE SALLALLAHOU…/JALLAJALALOUHOU
        | '\u{FE70}' | '\u{FE72}' | '\u{FE74}' | '\u{FE76}'
        | '\u{FE78}' | '\u{FE7A}' | '\u{FE7C}' | '\u{FE7E}' // ARABIC … ISOLATED FORM
        | '\u{FF9E}'..='\u{FF9F}'   // HALFWIDTH KATAKANA (SEMI-)VOICED SOUND MARK
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // `ID_Start \ XID_Start` — every code point must be a valid identifier start.
    const ID_START_NOT_XID: &[char] = &[
        '\u{037A}', '\u{0E33}', '\u{0EB3}', '\u{309B}', '\u{309C}', '\u{FC5E}', '\u{FC5F}',
        '\u{FC60}', '\u{FC61}', '\u{FC62}', '\u{FC63}', '\u{FDFA}', '\u{FDFB}', '\u{FE70}',
        '\u{FE72}', '\u{FE74}', '\u{FE76}', '\u{FE78}', '\u{FE7A}', '\u{FE7C}', '\u{FE7E}',
        '\u{FF9E}', '\u{FF9F}',
    ];

    // `ID_Continue \ XID_Continue` — the four start-only entries (U+0E33, U+0EB3,
    // U+FF9E, U+FF9F) are already in `XID_Continue`, so they're excluded here.
    const ID_CONTINUE_NOT_XID: &[char] = &[
        '\u{037A}', '\u{309B}', '\u{309C}', '\u{FC5E}', '\u{FC5F}', '\u{FC60}', '\u{FC61}',
        '\u{FC62}', '\u{FC63}', '\u{FDFA}', '\u{FDFB}', '\u{FE70}', '\u{FE72}', '\u{FE74}',
        '\u{FE76}', '\u{FE78}', '\u{FE7A}', '\u{FE7C}', '\u{FE7E}',
    ];

    #[test]
    fn id_start_includes_non_xid_code_points() {
        for &ch in ID_START_NOT_XID {
            assert!(
                is_id_start(ch),
                "ID_Start should accept U+{:04X}",
                ch as u32
            );
            assert!(
                is_id_continue(ch),
                "ID_Continue should accept U+{:04X}",
                ch as u32
            );
            // Pins the gap closed: these are exactly the points unicode_ident misses.
            assert!(
                !is_xid_start(ch),
                "U+{:04X} unexpectedly entered XID_Start",
                ch as u32
            );
        }
    }

    #[test]
    fn id_continue_gap_is_exactly_the_non_xid_continue_set() {
        // The continue gap is precisely the start gap minus the four already-XID points.
        for &ch in ID_CONTINUE_NOT_XID {
            assert!(
                !is_xid_continue(ch),
                "U+{:04X} unexpectedly entered XID_Continue",
                ch as u32
            );
        }
        for ch in [' ', '\u{2E2F}', '0', '!'] {
            // U+2E2F (VERTICAL TILDE) is category Lm but Pattern_Syntax, so NOT ID_Start —
            // the canonical char that separates the real property from a category-only check.
            assert!(
                !is_id_start(ch),
                "U+{:04X} must not be an identifier start",
                ch as u32
            );
        }
        // Digits are IdentifierPart but not IdentifierStart.
        assert!(is_id_continue('0') && !is_id_start('0'));
    }
}
