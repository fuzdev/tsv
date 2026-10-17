// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The pairs between a comma-list element holding a `<` / `<<` and a later element holding a
//! binary the parser flagged as able to close the type-argument region the `<` opens: a pair
//! the author wrote directly behind `typeof` or a unary `-` stays, and the printer adds none of
//! its own around an arrow function's conditional body, an assignment, or an arrow function's
//! lone parameter written bare (`docs/conformance_prettier_ts.md` §Relational chain
//! type-argument parens, "An authored pair behind `typeof` or a sign" and "No pair of the
//! printer's own between the two").
//!
//! The carriers, the positions, the controls and the width boundary are fixture claims
//! (`expressions/binary/relational_region_{prefix_shell,style_pair,arrow_param_pair}*_prettier_divergence`).
//! What is pinned here is what a fixture cannot hold: authorings tsc rejects as written (each
//! style pair written by hand), a trailing comma the printer drops, an input prettier rejects,
//! a frozen element on either side of the shape, comments in and around each pair, the plain-JS
//! reading, and the realistic lines the rules exist for. Every case is a fixed point whose
//! reparse keeps the program the source wrote.

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
fn an_authored_style_pair_stays() {
    // Each line is an authoring tsc rejects as written — the pair is what its recovering list
    // parse completes — and acorn-typescript and this parser read as the two comparisons. The
    // region follows the source, so a pair the author wrote prints as written: the printer
    // neither adds the pair nor strips one.
    for source in [
        "fn(a < b, (c) => (c ? d : e), f > (g, h));",
        "fn(a < b, (c = d), e > (f, g));",
        "fn(a < b, (c) => (d = e), f > (g, h));",
        "fn(a < b, [(c = d)], e > (f, g));",
        "fn(a < b, `${(c = d)}`, e > (f, g));",
        "fn(a < b, ([c] = d), e > (f, g));",
        "fn(a < b, (c) => (d) => (d ? e : f), g > (h, i));",
        "(a < b, (c = d), e > (f, g));",
        "const a2 = `${(a < b, (c) => (c ? d : e), f > (g, h))}`;",
        "fn(a < b, { p: (c = d) }, e > (f, g));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn an_assignment_target_keeps_its_shell() {
    // With the assignment's own pair left out, a stripped target shell would leave a bare name
    // where the author's `(` stood, which inside the authored sequence pair tsc's recovery
    // reads as a function type's parameters (`(y, c = d)`).
    for source in [
        "fn(a < b, (c) = d, e > (f, g));",
        "fn(a < b, (y, (c) = d) > (e, f));",
        "fn(a < b, (y, (c.d) = e), f > (g, h));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn a_pair_the_tree_requires_is_untouched() {
    // tsc rejects these as written too; the pairs are the grammar's, never the style's.
    for source in [
        "fn(a < b, () => ({ x: f() }), c > (d, e));",
        "fn(a < b, () => (c, d), e > (f, g));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn a_curried_callee_keeps_its_line() {
    // A conditional body the region leaves bare stays on the chain's head line, the line its
    // pair would have held it on, so the callee's own parens stay closed.
    for source in [
        "fn(a < b, ((c) => (d) => d ? x : f)(y), g > (h, i));",
        "fn(a < b, ((c) => (d) => (d ? x : f))(y), g > (h, i));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn a_shift_region_follows_the_flag() {
    // A `<<` opens two regions and a `>>` closes two, so the flag is set; the shell is kept
    // and the body left bare, though tsc reads these lines as comparisons either way.
    for source in [
        "fn(a << b, typeof (c) >> (d, e));",
        "fn(a << b, (c) => c ? d : e, f >> (g, h));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn a_nested_call_inside_a_region_element_follows_the_source() {
    // The walk descends into the inner call's argument; both spellings are the same program
    // to every parser, since tsc's list parse ends at the inner call's `(`. An arrow inside a
    // call argument breaks the outer list as everywhere else; the pairs follow the source.
    for (source, expected) in [
        (
            "fn(a < b, g((c) => c ? d : e), f > (h, i));",
            "fn(\n\ta < b,\n\tg((c) => c ? d : e),\n\tf > (h, i)\n);",
        ),
        (
            "fn(a < b, g((c) => (c ? d : e)), f > (h, i));",
            "fn(\n\ta < b,\n\tg((c) => (c ? d : e)),\n\tf > (h, i)\n);",
        ),
        (
            "fn(a < b, g(x => x), e > (f, h));",
            "fn(\n\ta < b,\n\tg(x => x),\n\te > (f, h)\n);",
        ),
        (
            "fn(a < b, g(c = d), e > (f, h));",
            "fn(a < b, g(c = d), e > (f, h));",
        ),
        (
            "fn(a < b, g((c = d)), e > (f, h));",
            "fn(a < b, g((c = d)), e > (f, h));",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn an_assignment_value_is_walked_for_its_own_pairs() {
    // The walk reaches into an assignment at a style position, so an arrow assigned there
    // takes no pair of the printer's own around its conditional body either.
    for source in [
        "fn(a < b, c = (d) => d ? e : f, g > (h, i));",
        "fn(a < b, c = (d) => (d ? e : f), g > (h, i));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn the_comparison_holder_is_part_of_the_region() {
    // Right of its `<`, the holder's own pairs follow the source as the later elements' do.
    for source in [
        "fn(a < b ? (c) => c ? d : e : 0, f > (g, h));",
        "fn(a < g(c = d), e > (f, g));",
        "fn(x = (b < c), e > (f, g));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn the_index_decides_not_a_trailing_comma() {
    // The trailing comma prints away; the closer's element is still the last one with a
    // closer, so the shape before it is still in the region.
    for (source, expected) in [
        (
            "fn(a < b, (c) => c ? d : e, f > (g, h),);",
            "fn(a < b, (c) => c ? d : e, f > (g, h));",
        ),
        (
            "fn(a < b, c = d, e > (f, g),);",
            "fn(a < b, c = d, e > (f, g));",
        ),
        (
            "fn(a < b, typeof (c), d > (e, f),);",
            "fn(a < b, typeof (c), d > (e, f));",
        ),
        (
            "fn(a < b, [c => c > (d, e)],);",
            "fn(a < b, [c => c > (d, e)]);",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_shape_inside_an_object_literal_follows_the_rule() {
    // `{ p: c = d }` is an input prettier rejects (to tsc's parser it is also a type literal
    // whose member has an initializer), so no fixture can hold it.
    for source in [
        "fn(a < b, { p: c = d }, e > (f, g));",
        "fn(a < b, { p: (c) => c ? d : e }, f > (g, h));",
        "fn(a < b, { p: c => c, q: d > (e, f) });",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn a_bare_arrow_parameter_stays_bare_and_an_authored_pair_stays() {
    for source in [
        // Behind `async`, ahead of the closer's element.
        "fn(a < b, [async c => c], d > (e, f));",
        // A list element of its own, between the two comparisons or holding the closer.
        "fn(a < b, c => c ? d : e, f > (g, h));",
        "fn(a < b, c => d, e > (f, g));",
        "fn(a < b, c => c > (d - e) * 2);",
        "fn(a < b, c /* k */ => c > (d, e));",
        // The author's pair around a parameter is kept, never stripped.
        "fn(a < b, (c) => c ? d : e, f > (g, h));",
        "fn(a < b, [(c) => c ? d : e], f > (g, h));",
    ] {
        assert_eq!(stable(source), source);
    }
}

#[test]
fn an_arrow_parameter_list_is_walked() {
    // A parameter's default is inside the element, so a shell behind `typeof` and a bare lone
    // parameter there follow the source too.
    for source in [
        "fn(a < b, (x = typeof (c)) => x, e > (f, g));",
        "fn(a < b, (x = y => y) => x, e > (f, g));",
    ] {
        assert_eq!(stable(source), source);
    }
}

/// The span-free wire of the component `source`, so a reparse must keep the same component.
fn svelte_tree(source: &str) -> Value {
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
    let root = tsv_svelte::parse(source, &arena).expect("the component parses");
    let mut value = tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&root, source));
    strip(&mut value);
    value
}

#[test]
fn a_function_binding_reads_its_operands_as_one_list() {
    // `bind:value={get, set}` prints its operands bare, so the region reaches across them as
    // across a sequence's — with no `<script>` and with `lang="ts"`, on a component, beside a
    // freeze and a comment in each gap, around an operand's own pair, and where the binding
    // takes more than two operands.
    for (source, expected) in [
        (
            "<input bind:value={() => lo < hi, v => v > (lo + hi) / 2 && set(v)} />",
            "<input bind:value={() => lo < hi, v => v > (lo + hi) / 2 && set(v)} />",
        ),
        (
            "<script lang=\"ts\">\n\tlet a = 0;\n</script>\n\n<input bind:value={a < b, typeof (c) > (d, e)} />",
            "<script lang=\"ts\">\n\tlet a = 0;\n</script>\n\n<input bind:value={a < b, typeof (c) > (d, e)} />",
        ),
        (
            "<input bind:value={a < b, -(1) > (d, e)} />",
            "<input bind:value={a < b, -(1) > (d, e)} />",
        ),
        (
            "<input bind:value={a < b, x = (c < d), e > (f, g)} />",
            "<input bind:value={a < b, x = (c < d), e > (f, g)} />",
        ),
        (
            "<Comp bind:p={a < b, c => c > (d, e)} />",
            "<Comp bind:p={a < b, c => c > (d, e)} />",
        ),
        (
            "<input bind:value={a < b,\n// prettier-ignore\n c  =>  c > (d, e)} />",
            "<input\n\tbind:value={\n\t\ta < b,\n\t\t// prettier-ignore\n\t\tc  =>  c > (d, e)\n\t}\n/>",
        ),
        (
            "<input bind:value={a < b, // k\n c => c > (d, e)} />",
            "<input\n\tbind:value={\n\t\ta < b, // k\n\t\tc => c > (d, e)\n\t}\n/>",
        ),
        (
            "<input bind:value={/* c */ a < b, c => c > (d, e)} />",
            "<input bind:value={/* c */ a < b, c => c > (d, e)} />",
        ),
        (
            "<input bind:value={a < b, c => c > (d, e) /* t */} />",
            "<input bind:value={a < b, c => c > (d, e) /* t */} />",
        ),
        (
            "<input bind:value={// h\n a < b, c => c > (d, e)} />",
            "<input\n\tbind:value={\n\t\t// h\n\t\ta < b, c => c > (d, e)\n\t}\n/>",
        ),
        // An operand's own pair: around a comparison, a comment inside it, a doubled pair,
        // and around an assignment operand, which a sequence prints in its style's pair.
        (
            "<input bind:value={(/* k */ a < b), c > (d, e)} />",
            "<input bind:value={(/* k */ a < b), c > (d, e)} />",
        ),
        (
            "<input bind:value={(a < b /* k */), c > (d, e)} />",
            "<input bind:value={(a < b) /* k */, c > (d, e)} />",
        ),
        (
            "<input bind:value={((a < b)), c > (d, e)} />",
            "<input bind:value={(a < b), c > (d, e)} />",
        ),
        (
            "<input bind:value={(x = a < b), c > (d, e)} />",
            "<input bind:value={(x = a < b), c > (d, e)} />",
        ),
        (
            "<input bind:value={a < b, (x = c > (d, e))} />",
            "<input bind:value={a < b, (x = c > (d, e))} />",
        ),
        (
            "<input bind:value={a < b, (x = y), e > (f, g)} />",
            "<input bind:value={a < b, (x = y), e > (f, g)} />",
        ),
        // The pair stays where no operand closes the region, and the host's own style rules
        // outside one.
        (
            "<input bind:value={a, (x = y)} />",
            "<input bind:value={a, x = y} />",
        ),
        (
            "<input bind:value={() => a < b, v => (a = v)} />",
            "<input bind:value={() => a < b, (v) => (a = v)} />",
        ),
    ] {
        let output = tsv_svelte::format_str(source).expect("the component formats");
        assert_eq!(
            tsv_svelte::format_str(&output).expect("the output formats"),
            output,
            "not a fixed point: {source}"
        );
        assert_eq!(
            svelte_tree(source),
            svelte_tree(&output),
            "the component changed: {source}"
        );
        assert_eq!(output.trim_end(), expected, "{source}");
    }
}

#[test]
fn the_prefix_shell_keeps_every_spelling_of_its_operand() {
    for (source, expected) in [
        // Glued to the operator, the pair prints with the operator's space.
        (
            "fn(a < b, typeof(c), d > (e, f));",
            "fn(a < b, typeof (c), d > (e, f));",
        ),
        (
            "fn(a < b, -(1n), d > (e, f));",
            "fn(a < b, -(1n), d > (e, f));",
        ),
        // A doubled pair prints once.
        (
            "fn(a < b, typeof ((c)) > (d, e));",
            "fn(a < b, typeof (c) > (d, e));",
        ),
        ("fn(a < b, -((1)) > (d, e));", "fn(a < b, -(1) > (d, e));"),
        // A pair around a longer head keeps it one parenthesized chain base, before a member,
        // a call or a `!`, and a call chain past it.
        (
            "fn(a < b, typeof (c.d)().e > (f, g));",
            "fn(a < b, typeof (c.d)().e > (f, g));",
        ),
        (
            "fn(a < b, typeof (c.d)!.e > (f, g));",
            "fn(a < b, typeof (c.d)!.e > (f, g));",
        ),
        (
            "fn(a < b, typeof (c.d)!.e().f > (f, g));",
            "fn(a < b, typeof (c.d)!.e().f > (f, g));",
        ),
        (
            "fn(a < b, typeof (c)().d.e().f > (f, g));",
            "fn(a < b, typeof (c)().d.e().f > (f, g));",
        ),
        ("fn(a < b, -(c)() > (f, g));", "fn(a < b, -(c)() > (f, g));"),
        (
            "fn(a < b, -(c.d).e > (f, g));",
            "fn(a < b, -(c.d).e > (f, g));",
        ),
        (
            "fn(a < b, typeof (c)?.() > (f, g));",
            "fn(a < b, typeof (c)?.() > (f, g));",
        ),
        // A comment inside the head's pair prints once, ahead of the head inside the pair and
        // past it outside.
        (
            "fn(a < b, typeof (/* k */ c.d).e > (f, g));",
            "fn(a < b, typeof (/* k */ c.d).e > (f, g));",
        ),
        (
            "fn(a < b, typeof (c.d /* k */).e > (f, g));",
            "fn(a < b, typeof (c.d) /* k */.e > (f, g));",
        ),
        (
            "fn(a < b, typeof (c.d /* k */)!.e > (f, g));",
            "fn(a < b, typeof (c.d /* k */)!.e > (f, g));",
        ),
        // A pair around the operand and one around its head inside it both stay.
        (
            "fn(a < b, typeof ((c)()), e > (f, g));",
            "fn(a < b, typeof ((c)()), e > (f, g));",
        ),
        (
            "fn(a < b, typeof ((c).d), e > (f, g));",
            "fn(a < b, typeof ((c).d), e > (f, g));",
        ),
        (
            "fn(a < b, typeof ((c)().d), e > (f, g));",
            "fn(a < b, typeof ((c)().d), e > (f, g));",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_comment_ahead_of_a_head_shell_stays_ahead_of_it() {
    // The comment between the operator and the head's pair prints in a pair of its own,
    // still glued to the head's `(`; that pair is the operand's shell on the next pass,
    // and the head's pair inside it stays, so the first output is the fixed point.
    for (source, expected) in [
        (
            "fn(a < b, typeof /* j */ (c)(), e > (f, g));",
            "fn(a < b, typeof (/* j */ (c)()), e > (f, g));",
        ),
        (
            "fn(a < b, typeof /* j */ (c.d).e, e > (f, g));",
            "fn(a < b, typeof (/* j */ (c.d).e), e > (f, g));",
        ),
        (
            "fn(a < b, typeof /* j */ (c)!, e > (f, g));",
            "fn(a < b, typeof (/* j */ (c)!), e > (f, g));",
        ),
        (
            "fn(a < b, typeof /* j */ (c)`t`, e > (f, g));",
            "fn(a < b, typeof (/* j */ (c)`t`), e > (f, g));",
        ),
        (
            "fn(a < b, typeof /* j */ (c)[d], e > (f, g));",
            "fn(a < b, typeof (/* j */ (c)[d]), e > (f, g));",
        ),
        (
            "fn(a < b, - /* j */ (c)(), e > (f, g));",
            "fn(a < b, -(/* j */ (c)()), e > (f, g));",
        ),
        // A head below a longer chain.
        (
            "fn(a < b, typeof /* j */ (c)().d, e > (f, g));",
            "fn(a < b, typeof (/* j */ (c)().d), e > (f, g));",
        ),
        (
            "fn(a < b, typeof /* j */ (c.d)!.e, e > (f, g));",
            "fn(a < b, typeof (/* j */ (c.d)!.e), e > (f, g));",
        ),
        // In the element holding the `<`.
        (
            "fn(a < typeof /* j */ (c)(), e > (f, g));",
            "fn(a < typeof (/* j */ (c)()), e > (f, g));",
        ),
        // A line comment opens the pair's line.
        (
            "fn(a < b, typeof // j\n(c)(), e > (f, g));",
            "fn(\n\ta < b,\n\ttypeof ( // j\n\t\t(c)()\n\t),\n\te > (f, g)\n);",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_frozen_element_on_either_side_of_the_shape() {
    for (source, expected) in [
        // The `<` element frozen: the parser's flag is the same, so the shape behind it
        // follows the rule.
        (
            "fn(\n// prettier-ignore\na  <  b, (c) => c ? d : e, f > (g, h));",
            "fn(\n\t// prettier-ignore\n\ta  <  b,\n\t(c) => c ? d : e,\n\tf > (g, h)\n);",
        ),
        (
            "fn(\n// prettier-ignore\na  <  b, c = d, e > (f, g));",
            "fn(\n\t// prettier-ignore\n\ta  <  b,\n\tc = d,\n\te > (f, g)\n);",
        ),
        (
            "fn(\n// prettier-ignore\na  <  b, typeof (c), d > (e, f));",
            "fn(\n\t// prettier-ignore\n\ta  <  b,\n\ttypeof (c),\n\td > (e, f)\n);",
        ),
        (
            "fn(\n// prettier-ignore\na  <  b, [x => x > (lo + hi) / 2]);",
            "fn(\n\t// prettier-ignore\n\ta  <  b,\n\t[x => x > (lo + hi) / 2]\n);",
        ),
        // The shape frozen: printed as written.
        (
            "fn(a < b,\n// prettier-ignore\nc  =  d, e > (f, g));",
            "fn(\n\ta < b,\n\t// prettier-ignore\n\tc  =  d,\n\te > (f, g)\n);",
        ),
        (
            "fn(a < b,\n// prettier-ignore\n[x  =>  x > (lo + hi) / 2]);",
            "fn(\n\ta < b,\n\t// prettier-ignore\n\t[x  =>  x > (lo + hi) / 2]\n);",
        ),
        (
            "fn(a < b,\n// prettier-ignore\n(c)  =>  c ? d : e, f > (g, h));",
            "fn(\n\ta < b,\n\t// prettier-ignore\n\t(c)  =>  c ? d : e,\n\tf > (g, h)\n);",
        ),
        (
            "fn(a < b,\n// prettier-ignore\ntypeof  (c), d > (e, f));",
            "fn(\n\ta < b,\n\t// prettier-ignore\n\ttypeof  (c),\n\td > (e, f)\n);",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_comment_in_or_around_each_pair_prints_once() {
    for (source, bare_or_kept) in [
        // The withheld body pair: a block comment leading the conditional, behind its test,
        // and a line comment behind the arrow's element.
        (
            "fn(a < b, (c) => /* k */ c ? d : e, f > (g, h));",
            "c ? d : e",
        ),
        (
            "fn(a < b, (c) => c /* k */ ? d : e, f > (g, h));",
            "? d : e,",
        ),
        (
            "fn(a < b, (c) => c ? d : e, // k\nf > (g, h));",
            "=> c ? d : e",
        ),
        // The withheld assignment pair.
        ("fn(a < b, c = /* k */ d, e > (f, g));", "c = /* k */ d,"),
        // The kept prefix shell, with the comment inside it and ahead of it.
        ("fn(a < b, typeof (/* k */ c) > (d, e));", "typeof ("),
        ("fn(a < b, typeof (c /* k */) > (d, e));", "typeof ("),
        ("fn(a < b, typeof /* k */ (c) > (d, e));", "c) > (d, e)"),
        // The bare parameter.
        ("fn(a < b, [x /* k */ => x > (lo + hi) / 2]);", "[x"),
    ] {
        let output = stable(source);
        assert_eq!(output.matches('k').count(), 1, "{source}: {output}");
        assert!(output.contains(bare_or_kept), "{source}: {output}");
        assert!(
            !output.contains("=> (") && !output.contains("(c = "),
            "{output}"
        );
    }
}

#[test]
fn a_plain_script_reads_the_same_lines() {
    // A `<script>` with no `lang="ts"` parses with the same parser, and the lines are the same
    // two comparisons there.
    for line in [
        "fn(a < b, (c) => c ? d : e, f > (g, h));",
        "fn(a < b, c = d, e > (f, g));",
        "fn(a < b, typeof (c) > (d, e));",
        "check(lo < hi, [x => x > (lo + hi) / 2]);",
    ] {
        let source = format!("<script>\n\t{line}\n</script>\n");
        let output = tsv_svelte::format_str(&source).expect("the component formats");
        assert_eq!(output, source);
    }
}

#[test]
fn realistic_lines_keep_their_program() {
    for (source, expected) in [
        // A `typeof(x)` logged as its own argument.
        (
            "console.log(i < n, typeof(v), v > (lo + hi) / 2);",
            "console.log(i < n, typeof (v), v > (lo + hi) / 2);",
        ),
        (
            "debug(count < limit, typeof (payload), payload.size > (limit - count) * 2);",
            "debug(count < limit, typeof (payload), payload.size > (limit - count) * 2);",
        ),
        (
            "log(x < y, typeof(value), value > (max ?? 0));",
            "log(x < y, typeof (value), value > (max ?? 0));",
        ),
        (
            "const row = [i < len, typeof(arr[i]), arr[i] > (min + max) / 2];",
            "const row = [i < len, typeof (arr[i]), arr[i] > (min + max) / 2];",
        ),
        (
            "expect.soft(lo < hi, typeof (mid), mid > (lo + hi) / 2);",
            "expect.soft(lo < hi, typeof (mid), mid > (lo + hi) / 2);",
        ),
        (
            "new Probe(lo < hi, typeof(lo), hi > (lo + 1) * 2);",
            "new Probe(lo < hi, typeof (lo), hi > (lo + 1) * 2);",
        ),
        (
            "trace(a < b, typeof(this), b > (a + pad) * 2);",
            "trace(a < b, typeof (this), b > (a + pad) * 2);",
        ),
        (
            "trace(a < b, typeof (opts.mode), b > (a + pad) * 2);",
            "trace(a < b, typeof (opts.mode), b > (a + pad) * 2);",
        ),
        (
            "report(w < min, typeof(w), w > `${max}px`);",
            "report(w < min, typeof (w), w > `${max}px`);",
        ),
        // A negated literal.
        (
            "range(i < n, -(1), n > (i + 1) * 2);",
            "range(i < n, -(1), n > (i + 1) * 2);",
        ),
        // A callback that picks between two names.
        (
            "check(lo < hi, (v) => v ? yes : no, x > (lo + hi) / 2);",
            "check(lo < hi, (v) => v ? yes : no, x > (lo + hi) / 2);",
        ),
        (
            "validate(min < max, (ok) => ok ? value : undefined, value > (min + max) / 2);",
            "validate(min < max, (ok) => ok ? value : undefined, value > (min + max) / 2);",
        ),
        (
            "assert(a < b, (err) => err ? onError : onOk, c > (a + b) / 2);",
            "assert(a < b, (err) => err ? onError : onOk, c > (a + b) / 2);",
        ),
        (
            "pick(i < n, (flag) => flag ? left : right, n > (i + 1) * 2);",
            "pick(i < n, (flag) => flag ? left : right, n > (i + 1) * 2);",
        ),
        (
            "fn(x < 0, (v) => v ? a : null, x > (limit - 1) * 2);",
            "fn(x < 0, (v) => v ? a : null, x > (limit - 1) * 2);",
        ),
        (
            "check(lo < hi, () => enabled ? on : off, x > (lo + hi) / 2);",
            "check(lo < hi, () => enabled ? on : off, x > (lo + hi) / 2);",
        ),
        (
            "check(lo < hi, (v: number) => v ? yes : no, x > (lo + hi) / 2);",
            "check(lo < hi, (v: number) => v ? yes : no, x > (lo + hi) / 2);",
        ),
        (
            "check(lo < hi, (v, i) => v ? yes : no, x > (lo + hi) / 2);",
            "check(lo < hi, (v, i) => v ? yes : no, x > (lo + hi) / 2);",
        ),
        (
            "const row = [a < b, (v) => v ? x : y, c > (d + e) / 2];",
            "const row = [a < b, (v) => v ? x : y, c > (d + e) / 2];",
        ),
        (
            "new Check(lo < hi, (v) => v ? yes : no, x > (lo + hi) / 2);",
            "new Check(lo < hi, (v) => v ? yes : no, x > (lo + hi) / 2);",
        ),
        (
            "check(lo < hi, { test: (v) => v ? yes : no }, x > (lo + hi) / 2);",
            "check(lo < hi, { test: (v) => v ? yes : no }, x > (lo + hi) / 2);",
        ),
        (
            "check(lo < hi, [(v) => v ? yes : no], x > (lo + hi) / 2);",
            "check(lo < hi, [(v) => v ? yes : no], x > (lo + hi) / 2);",
        ),
        (
            "check(lo < hi, (v) => v ? yes : no, x > `${lo}`);",
            "check(lo < hi, (v) => v ? yes : no, x > `${lo}`);",
        ),
        (
            "items.map(i < n, (it) => it ? it : fallback, n > (i + 1) * 2);",
            "items.map(i < n, (it) => it ? it : fallback, n > (i + 1) * 2);",
        ),
        // An assignment.
        (
            "on(i < n, (v) => last = v, n > (i + 1) * 2);",
            "on(i < n, (v) => last = v, n > (i + 1) * 2);",
        ),
        (
            "watch(prev < next, (v) => value = v, next > (prev + step) * 2);",
            "watch(prev < next, (v) => value = v, next > (prev + step) * 2);",
        ),
        (
            "check(lo < hi, x = mid, x > (lo + hi) / 2);",
            "check(lo < hi, x = mid, x > (lo + hi) / 2);",
        ),
        (
            "on(i < n, (v) => total += v, n > (i + 1) * 2);",
            "on(i < n, (v) => total += v, n > (i + 1) * 2);",
        ),
        (
            "on(i < n, (v) => state.last = v, n > (i + 1) * 2);",
            "on(i < n, (v) => state.last = v, n > (i + 1) * 2);",
        ),
        // A predicate in an array or an object beside the bound.
        (
            "check(lo < hi, [x => x > (lo + hi) / 2]);",
            "check(lo < hi, [x => x > (lo + hi) / 2]);",
        ),
        (
            "filter(min < max, { keep: v => v > (min + max) / 2 });",
            "filter(min < max, { keep: v => v > (min + max) / 2 });",
        ),
        (
            "const rules = [lo < hi, [v => v, hi > (lo + 1) * 2]];",
            "const rules = [lo < hi, [v => v, hi > (lo + 1) * 2]];",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}
