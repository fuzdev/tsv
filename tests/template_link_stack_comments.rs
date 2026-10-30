// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A comment inside a member or a `!` printed over a call whose sole argument is a multiline
//! template on the `(` line — the shape prettier prints as the object whole, then the lookup —
//! prints exactly once, in a single-pass fixed point. The layout of these lines is not prettier's
//! (a commented one keeps the member-chain form), so no row pins it: the rows hold the comment
//! count, which no fixture can, since a fixture input must be prettier's own form.

fn format(source: &str) -> String {
    tsv_ts::format(source).expect("formats")
}

/// One comment in each gap a stack of template links has: between two links, ahead of the
/// lookup, around a `!`, inside and after the pair around a sealed optional chain, on its own
/// line, spanning lines, ahead of a lookup that breaks, and under an assignment target and a
/// `new` callee.
const CASES: &[&str] = &[
    "v.m(r).o(`\na\n`) /* c */.o(`\nb\n`).p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`) /* c */.p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`) /* c */!.p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`)! /* c */.p;\n",
    "(/* c */ v?.o(`\na\n`)).p;\n",
    "(v?.o(`\na\n`) /* c */).p;\n",
    "(v?.o(`\na\n`)) /* c */.p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`)\n/* c */\n.p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`) /* c\n d */.p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`) /* c */.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`) /* c */.p = 1;\n",
    "new (v.m(r).o(`\na\n`).o(`\nb\n`) /* c */.p)();\n",
    "v.m(r).o(`\na\n`) // c\n.o(`\nb\n`).p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`) // c\n.p;\n",
    "v.m(r).o(`\na\n`).o(`\nb\n`)! // c\n.p;\n",
];

#[test]
fn every_comment_prints_once() {
    for source in CASES {
        let output = format(source);
        assert_eq!(format(&output), output, "not a fixed point: {source:?}");
        for comment in ["/* c */", "/* c\n d */", "// c"] {
            assert_eq!(
                output.matches(comment).count(),
                source.matches(comment).count(),
                "{comment:?} in {source:?} printed as {output:?}"
            );
        }
    }
}
