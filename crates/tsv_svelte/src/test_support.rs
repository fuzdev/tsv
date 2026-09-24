//! Helpers shared by the crate's unit tests.

/// Call `check` on every string of up to `max_len` pieces drawn from `pieces`, the empty
/// string included — the exhaustive enumerator behind the tests that grade a byte-walk
/// predicate against the reference spelling it replaces, at every arrangement of the
/// characters it keys on rather than the few a corpus spells.
pub(crate) fn for_every_arrangement(pieces: &[&str], max_len: u32, mut check: impl FnMut(&str)) {
    let mut s = String::new();
    for len in 0..=max_len {
        for code in 0..pieces.len().pow(len) {
            s.clear();
            let mut c = code;
            for _ in 0..len {
                s.push_str(pieces[c % pieces.len()]);
                c /= pieces.len();
            }
            check(&s);
        }
    }
}
