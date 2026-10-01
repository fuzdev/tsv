// helper fns here aren't `#[test]`, so clippy.toml's allow-expect/panic-in-tests don't reach them
#![expect(clippy::expect_used, clippy::panic)]

//! An identifier name too long for `IdentName`'s `raw_len` (a `u16`) keeps its text as
//! the arena-copied escape hatch instead of as a length over the node's span, and this
//! file grades that arm at the boundary: a name of 65535 bytes is the longest the length
//! holds, 65536 and 65537 are the first two that take the copy.
//!
//! No corpus reaches it — no real identifier is 64 KiB long — so nothing else notices
//! the arm handing back the wrong text: an empty name still parses, still formats and
//! still writes a well-formed wire. So each case reads the name back off both wire
//! variants and off the formatted output, at every position the name was written in.

/// The name positions graded, as templates with `NAME` standing in for the identifier
/// and the number of name fields each puts on the wire: a binding, a member property
/// and a `#private` member.
const POSITIONS: &[(&str, usize)] = &[
    ("const NAME = 1;\n", 1),
    ("x.NAME;\n", 1),
    ("class C {\n\t#NAME = 1;\n}\n", 1),
];

/// The byte lengths either side of the `u16` boundary.
const LENGTHS: [usize; 3] = [65535, 65536, 65537];

/// A plain-ASCII identifier of exactly `len` bytes.
fn ascii_name(len: usize) -> String {
    "a".repeat(len)
}

/// A non-ASCII identifier of exactly `len` bytes: two-byte `é`s, behind one `a` when
/// `len` is odd.
fn wide_name(len: usize) -> String {
    let mut name = "a".repeat(len % 2);
    name.push_str(&"\u{e9}".repeat(len / 2));
    assert_eq!(name.len(), len);
    name
}

/// How many times `needle` occurs in `haystack`.
fn count(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

/// Grade one name at every position: both wire variants carry it in each name field
/// the source holds, and the formatted output keeps it and is a fixed point.
#[track_caller]
fn assert_name_survives(name: &str) {
    let len = name.len();
    for &(template, hits) in POSITIONS {
        let source = template.replace("NAME", name);

        let arena = bumpalo::Bump::new();
        let program = tsv_ts::parse(&source, &arena)
            .unwrap_or_else(|e| panic!("parse failed for a {len}-byte name in {template:?}: {e}"));
        // A name of ASCII letters or `é` is written to the wire as its own bytes.
        let field = format!("\"name\":\"{name}\"");
        for (variant, bytes) in [
            ("default", tsv_ts::convert_ast_json_bytes(&program, &source)),
            (
                "no-locations",
                tsv_ts::convert_ast_json_bytes_no_locations(&program, &source),
            ),
        ] {
            let wire = String::from_utf8(bytes).expect("the wire is UTF-8");
            assert_eq!(
                count(&wire, &field),
                hits,
                "{len}-byte name in {template:?}: name fields on the {variant} wire"
            );
        }

        let formatted = tsv_ts::format_str(&source)
            .unwrap_or_else(|e| panic!("format failed for a {len}-byte name in {template:?}: {e}"));
        assert_eq!(
            count(&formatted, name),
            1,
            "{len}-byte name in {template:?}: the formatted output keeps the name"
        );
        let again = tsv_ts::format_str(&formatted).unwrap_or_else(|e| {
            panic!("reformat failed for a {len}-byte name in {template:?}: {e}")
        });
        assert!(
            again == formatted,
            "{len}-byte name in {template:?}: format is not idempotent"
        );
    }
}

#[test]
fn ascii_name_survives_at_the_raw_len_boundary() {
    for len in LENGTHS {
        assert_name_survives(&ascii_name(len));
    }
}

#[test]
fn non_ascii_name_survives_at_the_raw_len_boundary() {
    for len in LENGTHS {
        assert_name_survives(&wide_name(len));
    }
}
