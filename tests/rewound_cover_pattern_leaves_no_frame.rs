//! A conditional consequent read first as an arrow head refines its parameter, and the
//! refinement fails part-way through the pattern's list (a rest that is not last); the
//! rewind must close that list's frame, since the parenthesized reading then succeeds.

#[test]
fn a_rewound_cover_pattern_leaves_no_open_frame() {
    let source = "c ? ({ a, ...b, d }) : e => f;\n";
    let formatted = tsv_ts::format(source).expect("the parenthesized reading parses");
    assert_eq!(formatted, "c ? { a, ...b, d } : (e) => f;\n");
}
