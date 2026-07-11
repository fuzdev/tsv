//! Small source-scan helpers shared by the binder and the check pass.
//!
//! A computed member name (`[ … ]`) points its diagnostic at the whole
//! `ComputedPropertyName` node — bracket-inclusive — in tsgo. Both the bind
//! cascade (`binder::sym::resolve_member_key`) and the syntactic check
//! (`check::duplicate_members::member_key`) need the same `[`…`]` bounds so a
//! computed-literal key that conflicts at *both* phases produces byte-identical
//! spans that collapse in the program-wide sort/dedup (rather than two
//! differently-spanned diagnostics that survive as an extra). Lifting the scan
//! here keeps that agreement structural, not hand-mirrored.
//
// tsgo: internal/checker/checker.go reportDuplicateMemberErrors — the squiggle is
//       getNameOfDeclaration -> the ComputedPropertyName node (bracket-inclusive).

/// The byte offset of the `[` opening a computed key, scanning back from the key
/// expression's start (a plain byte loop — `[` is ASCII). Falls back to the
/// expression start if no `[` precedes it (never for a well-formed computed name).
#[must_use]
pub(crate) fn bracket_start(source: &str, expr_start: u32) -> u32 {
    let bytes = source.as_bytes();
    let mut i = expr_start as usize;
    while i > 0 {
        i -= 1;
        if bytes[i] == b'[' {
            return i as u32;
        }
    }
    expr_start
}

/// The byte offset just past the `]` closing a computed key, scanning forward from
/// the key expression's end (a plain byte loop — `]` is ASCII). Mirrors
/// [`bracket_start`] so the diagnostic spans the whole `[ … ]` name node (as tsgo
/// does). Falls back to the expression end if no `]` follows (never for a
/// well-formed computed name).
#[must_use]
pub(crate) fn bracket_end(source: &str, expr_end: u32) -> u32 {
    let bytes = source.as_bytes();
    let mut i = expr_end as usize;
    while i < bytes.len() {
        if bytes[i] == b']' {
            return i as u32 + 1;
        }
        i += 1;
    }
    expr_end
}
