//! A `//` written after a `for` header's update clause, where the update ends in a pair it
//! prints (one the printer adds ahead of `as`, or the one a non-null `!` requires) around a
//! stripped shell holding an own-line block: the output must be its own format.
//!
//! The header's trailing emitter defers a same-line `//` only when the update itself holds a
//! `//`, which its emitters may have deferred ahead of it. Deferred here, the comment would
//! flush behind the pending block, onto the block's line, and the next pass would read it as a
//! line of its own. The order these print in is not graded here, only that one pass settles.

/// Updates whose last pair holds an own-line block, each followed by a `//` after the clause.
const CASES: &[&str] = &[
    "for (; ; a + (b\n/* c */\n)! // d\n) {}\n",
    "for (; ; (a + (b\n/* c */\n)) as T // d\n) {}\n",
    "for (; ; (a + (b\n/* c\n */\n)) as T // d\n) {}\n",
];

#[test]
fn for_update_trailing_line_comment_after_a_kept_pair_is_idempotent() {
    let mut failures = Vec::new();
    for source in CASES {
        let once = tsv_ts::format(source).expect("the case formats");
        let twice = tsv_ts::format(&once).expect("the output formats");
        if once != twice {
            failures.push(format!(
                "{source:?}:\n  pass 1 {once:?}\n  pass 2 {twice:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} unstable cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
