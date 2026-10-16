// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The authored pair a comma list keeps around an element holding a `<` / `<<` when a later
//! element holds a binary the parser flagged as able to close the type-argument region the
//! `<` opens (`Printer::mark_relational_sibling_authored_pairs`,
//! `docs/conformance_prettier_ts.md` §Relational chain type-argument parens, "An authored pair
//! around an earlier comma sibling").
//!
//! The carrier sweep, the width boundary and the comment placements around the kept pair are
//! fixture claims (`expressions/binary/relational_sibling_authored_pair*_prettier_divergence`).
//! What is pinned here is what a fixture would carry only as a pile of near-identical cells:
//! the owner walk's boundaries one shape per rule, each comment placement at each kind of
//! list, a frozen element, the plain-JS reading, and the realistic lines the rule exists for.
//! Every case is a fixed point whose reparse keeps the program the source wrote.

use serde_json::Value;

/// The span-free wire of `source`, so a reparse must keep the same program.
fn tree(source: &str) -> Value {
    fn strip(node: &mut Value) {
        match node {
            Value::Object(map) => {
                for key in ["start", "end", "loc", "name_loc"] {
                    map.remove(key);
                }
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("the case parses");
    let mut value = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    strip(&mut value);
    value
}

/// `source` formatted, asserted a fixed point whose reparse is the program `source` wrote.
#[track_caller]
fn stable(source: &str) -> String {
    let output = tsv_ts::format_str(source).expect("the case formats");
    assert_eq!(
        tsv_ts::format_str(&output).expect("the output formats"),
        output,
        "not a fixed point: {source}"
    );
    assert_eq!(tree(source), tree(&output), "the program changed: {source}");
    output.trim_end().to_owned()
}

#[test]
fn every_comma_list_owner_keeps_the_pair() {
    for source in [
        // The call family: plain, `new`, optional, a method, a chain head, a decorator.
        "fn((a < b), c > (d, e));",
        "new Cls((a < b), c > (d, e));",
        "fn?.((a < b), c > (d, e));",
        "obj.fn((a < b), c > (d, e));",
        "obj.fn((a < b), c > (d, e)).then(x);",
        "@dec((a < b), c > (d, e))\nclass A {}",
        // `import()`'s two arguments are a list too: the source holds the pair, the options
        // the closer.
        "import((a < b), c > (d, e));",
        // An array element, a sequence statement, a `for` header's clause, and a sequence in
        // a template substitution.
        "const a1 = [(a < b), c > (d, e)];",
        "((a < b), c > (d, e));",
        "for ((a < b), c > (d, e); ;) {}",
        "const a2 = `${((a < b), c > (d, e))}`;",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn the_pair_and_its_closer_stand_anywhere_in_the_list() {
    for source in [
        // An element ahead of the pair, between it and the closer, behind the closer.
        "fn(x, (a < b), c > (d, e));",
        "fn((a < b), y, c > (d, e));",
        "fn((a < b), c > (d, e), y);",
        // Two paired elements ahead of one closer each keep their own.
        "fn((a < b), (c < d), e > (f, g));",
        // A `<<` opens a region a `>>` closes, and a `<` chain's two regions one `>>`.
        "fn((a << b), c >> (d, e));",
        "fn((a < b < c), d >> (e, f));",
        // The closer deeper in its element: an arrow's concise body, an array, an object.
        "fn((a < b), (c) => c > (d - e) * 2);",
        "fn((a < b), [c > (d, e)]);",
        "fn((a < b), { p: c > (d, e) });",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn a_pair_is_kept_never_added_and_only_ahead_of_a_flagged_closer() {
    for (source, expected) in [
        // A doubled pair prints once; a redundant pair inside the closer still strips.
        ("fn(((a < b)), c > d);", "fn((a < b), c > d);"),
        ("fn((a < b), c > (d));", "fn((a < b), c > d);"),
        // No later closer: `c`, an arrow, `>=`, and a closer only AHEAD of the comparison.
        ("fn((a < b), c);", "fn(a < b, c);"),
        ("fn((a < b), () => c);", "fn(a < b, () => c);"),
        ("fn((a < b), c >= d);", "fn(a < b, c >= d);"),
        ("fn(c > (d, e), (a < b));", "fn(c > (d, e), a < b);"),
        // No region opened: `<=`, arithmetic, a `<` a nested call's `)` already closes.
        ("fn((a <= b), c > (d, e));", "fn(a <= b, c > (d, e));"),
        ("fn((a + b), c > (d, e));", "fn(a + b, c > (d, e));"),
        ("fn((g(a < b)), c > (d, e));", "fn(g(a < b), c > (d, e));"),
        // A `>>` closes two regions, so one `<` leaves it open.
        ("fn((a < b), c >> (d, e));", "fn(a < b, c >> (d, e));"),
        // The LAST element is never paired: a trailing comma prints away, so the index
        // decides, not the byte behind the element.
        ("fn((a < b),);", "fn(a < b);"),
        ("fn((a < b), c > (d, e),);", "fn((a < b), c > (d, e));"),
        // Nothing is synthesized where the author wrote no pair, and a generic call written
        // as one stays one.
        ("fn(a < b + 1, c > (d, e));", "fn(a < b + 1, c > (d, e));"),
        ("fn(a<b, c>(d, e));", "fn(a<b, c>(d, e));"),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn an_element_that_prints_its_own_pair_takes_no_second() {
    for (source, expected) in [
        // An assignment's clarity pair and a sequence's own pair already end the element.
        ("fn((a = b), c > (d, e));", "fn((a = b), c > (d, e));"),
        ("fn(((a, b)), c > (d, e));", "fn((a, b), c > (d, e));"),
        // A `for` header's `[~In]` pair around the element is the kept pair, printed once.
        (
            "for (a, (x < y || p in q), c > (d, e); ;) {}",
            "for (a, (x < y || (p in q)), c > (d, e); ;) {}",
        ),
        (
            "for (import((x < y || p in q), c > (d, e)); ;) {}",
            "for (import((x < y || (p in q)), c > (d, e)); ;) {}",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_body_ends_the_walk_for_a_closer() {
    for (source, expected) in [
        // A statement or member inside a body is not an expression child of the element.
        (
            "fn((a < b), () => { return c > (d, e); });",
            "fn(a < b, () => {\n\treturn c > (d, e);\n});",
        ),
        (
            "fn((a < b), function () { return c > (d, e); });",
            "fn(a < b, function () {\n\treturn c > (d, e);\n});",
        ),
        (
            "fn((a < b), class { p = c > (d, e); });",
            "fn(\n\ta < b,\n\tclass {\n\t\tp = c > (d, e);\n\t}\n);",
        ),
        (
            "fn((a < b), { m() { return c > (d, e); } });",
            "fn(a < b, {\n\tm() {\n\t\treturn c > (d, e);\n\t}\n});",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn each_comment_placement_prints_once_at_each_list_kind() {
    // A block leading the comparison, a block behind it, a `//` behind it, and a `//`
    // glued to the pair's own `(` — at an argument, an array element and a sequence operand.
    for pair in [
        "(/* k */ a < b)",
        "(a < b /* k */)",
        "(a < b // k\n)",
        "( // k\na < b)",
    ] {
        for source in [
            format!("fn({pair}, c > (d, e));"),
            format!("[{pair}, c > (d, e)];"),
            format!("({pair}, c > (d, e));"),
        ] {
            let output = stable(&source);
            assert_eq!(output.matches('k').count(), 1, "{source}: {output}");
            assert!(
                output.contains("a < b") && output.contains("c > (d, e)"),
                "{output}"
            );
        }
    }
}

#[test]
fn a_frozen_element_prints_its_pair_once() {
    for (source, expected) in [
        (
            "fn(\n// prettier-ignore\n(a  <  b), c > (d, e));",
            "fn(\n\t// prettier-ignore\n\t(a  <  b),\n\tc > (d, e)\n);",
        ),
        (
            "import(\n// prettier-ignore\n(a  <  b), c > (d, e));",
            "import(\n\t// prettier-ignore\n\t(a  <  b),\n\tc > (d, e)\n);",
        ),
        // The frozen slice hands the header a bare `in`, and the kept pair is the one the
        // header's `[~In]` rule asks for.
        (
            "for (a,\n// prettier-ignore\n(x < y || p in q), c > (d, e); ;) {}",
            "for (\n\ta,\n\t\t// prettier-ignore\n\t\t(x < y || p in q),\n\t\tc > (d, e);\n\t;\n) {}",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_plain_script_reads_the_same_lines() {
    // A `<script>` with no `lang="ts"` parses with the same parser, so the bare spelling is
    // the same generic call there and the pair stays for the same reason.
    let source = "<script>\n\tfn((a < b), c > (d, e));\n</script>\n";
    let output = tsv_svelte::format_str(source).expect("the component formats");
    assert_eq!(output, source);
}

#[test]
fn a_paired_comparison_escapes_a_bare_arrow_parameter() {
    // The arrow parameter takes its own pair on print; the kept pair ahead of it is what
    // keeps the line two arguments.
    assert_eq!(
        stable("check((lo < hi), x => x > (lo + hi) / 2);"),
        "check((lo < hi), (x) => x > (lo + hi) / 2);"
    );
}

#[test]
fn realistic_range_checks_keep_their_program() {
    for source in [
        // Assertions over a range and a step.
        "assert((lo < hi), max > (hi - lo) * step);",
        "assert((index < items.length), count > (offset + limit) * 2);",
        "expect((start < end), total > (end - start) / size);",
        "console.assert((min < max), value > (max - min) * ratio);",
        "ok((a < b), c > (a + b) / 2);",
        "invariant((left < right), width > (right - left) * scale, 'bad range');",
        // Predicates handed a bound.
        "check((lo < hi), x => x > (lo + hi) / 2);",
        "check((lo < hi), (x) => x > (lo + hi) / 2);",
        "validate((min < max), value > (min + max) / 2 ? 1 : 0);",
        "clamp((x < lo), x > (hi - 1), lo, hi);",
        // Flags collected in arrays.
        "const flags = [(x < 0), x > (w - 1) / 2];",
        "const bounds = [(top < 0), bottom > (height - pad) * 2, (left < 0), right > (width - pad) * 2];",
        "return [(a < b), b > (a + 1) * 2];",
        "const hit = [(px < x0), px > (x0 + w) - 1].some(Boolean);",
        // Other carriers of the same line.
        "const r = new Range((lo < hi), hi > (lo + span) * 2);",
        "logger.debug((a < b), a > (b - 1) * 2);",
        "cb?.((n < 0), n > (max - 1) * 2);",
        "for (i = 0, (j < n), k > (n - 1) * 2; ; ) {}",
        "const s = `${((a < b), a > (b - 1) * 2)}`;",
        // Followers the rule never reads, and the operands it does not care about.
        "assert((lo < hi), label > `t`);",
        "assert((lo < hi), max > (hi, lo));",
        "assert((lo < hi), max > /r/.test(s));",
        "assert((lo < hi), max > hi - lo);",
        "assert((lo < hi), max > (hi));",
        "assert((lo < hi), max >= (hi - lo) * step);",
        "assert((lo < hi), max > (await next()));",
        "assert(ready, (lo < hi), max > (hi - lo) * step);",
        "assert((lo < hi), 'range', max > (hi - lo) * step);",
        "assert((a.lo < a.hi), a.max > (a.hi - a.lo) * a.step);",
        "assert((lo < 10), max > (hi - lo) * step);",
        "assert((items.length < limit), this.size > (limit - 1) * 2);",
    ] {
        stable(source);
    }
}

#[test]
fn a_blank_line_inside_a_kept_pair_keeps_the_comparisons() {
    // TODO: a blank line the author left inside the kept pair is not a fixed point: the
    // list's blank-line rule reads it as a blank between the elements, so the first pass
    // opens the list around it and the second, with the blank now really between them,
    // collapses it again. The same instability holds for an assignment's pair
    // (`fn((a = b⏎⏎), c)`). Both passes keep the two comparisons; the rows pin that, and the
    // two layouts as they stand, until the blank rule reads the pair.
    for (source, first, second) in [
        (
            "fn((a < b\n\n), (c) => c > (d - e) * 2);",
            "fn(\n\t(a < b),\n\n\t(c) => c > (d - e) * 2\n);\n",
            "fn((a < b), (c) => c > (d - e) * 2);\n",
        ),
        (
            "fn((a < b\n\n), [c > (d, e)]);",
            "fn(\n\t(a < b),\n\n\t[c > (d, e)]\n);\n",
            "fn((a < b), [c > (d, e)]);\n",
        ),
        (
            "fn((a = b\n\n), c);",
            "fn(\n\t(a = b),\n\n\tc\n);\n",
            "fn((a = b), c);\n",
        ),
    ] {
        let once = tsv_ts::format_str(source).expect("the case formats");
        assert_eq!(once, first, "{source}");
        assert_eq!(tree(source), tree(&once), "the program changed: {source}");
        let twice = tsv_ts::format_str(&once).expect("the output formats");
        assert_eq!(twice, second, "{source}");
        assert_eq!(stable(&twice), second.trim_end());
    }
}
