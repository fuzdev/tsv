//! The fused-span answers graded against the general builders they stand in for.
//!
//! A builder that emits one span for a construct printing as its own source bytes has no
//! output of its own: with the answer declined ([`super::without_fused_source_spans`]) the
//! general builder prints the same document, byte for byte. These tests hold that over the
//! fixture trees and over a generated matrix that walks each gate's boundary — a byte of
//! whitespace, a comment, a quote or a wide character inserted at every position of a
//! construct, at line widths either side of the print width.

use super::without_fused_source_spans;
use crate::format_str;
use std::path::{Path, PathBuf};

/// `source` formatted with the fused answers on and with them declined: the two must
/// agree, on a parse error as much as on an output.
fn assert_fused_matches_general(source: &str, what: &dyn std::fmt::Display) {
    let fused = format_str(source).map_err(|e| e.to_string());
    let general = without_fused_source_spans(|| format_str(source).map_err(|e| e.to_string()));
    assert_eq!(fused, general, "fused and general output differ for {what}");
}

fn collect_svelte_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_svelte_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "svelte") {
            out.push(path);
        }
    }
}

/// Every `.svelte` file of both fixture trees — inputs, `unformatted_*` variants and
/// prettier outputs alike — formats identically either way.
#[test]
fn fixture_trees_format_identically_without_fused_spans() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests");
    let mut files = Vec::new();
    collect_svelte_files(&tests.join("fixtures"), &mut files);
    collect_svelte_files(&tests.join("fixtures_compile"), &mut files);
    assert!(files.len() > 1000, "the fixture trees were not found");
    for path in &files {
        // A fixture that is not UTF-8 is not this test's to read.
        if let Ok(source) = std::fs::read_to_string(path) {
            assert_fused_matches_general(&source, &path.display());
        }
    }
}

/// What the matrix inserts at each position: the bytes each gate has to tell apart from
/// the contiguous plain-ASCII spelling it answers.
const INSERTS: &[&str] = &[
    " ",
    "\n",
    "\t",
    "/* c */",
    "/** c */",
    "// c\n",
    "<!-- c -->",
    "\"",
    "'",
    "=",
    "(",
    ")",
    "!",
    "?",
    ".",
    "#",
    "\\u0061",
    "\u{e9}",
    "\u{301}",
    "\u{1f600}",
    "\u{600}",
    "&amp;",
];

/// The constructs the fused answers cover, each beside the shapes its gate must decline.
const SEEDS: &[&str] = &[
    "<div class=\"a b\" title=\"x y\" data-n=\"\">t</div>",
    "<div class=\"a  b \" id='x' lang=en title=\"it's\">t</div>",
    "<Foo bar=\"baz\" {qux} quux={quux} a={b} c={d.e} f={g.h.i} j=\"{k}\" />",
    "<p>{a} {b.c} {d.e.f} {this.g} {h?.i} {j[0]} {await} {k.class} {m[n]}</p>",
    "<input bind:value={v} on:click={h} class:on={on} use:act={o.p} style:color={c} />",
    "{#if a}{b}{:else if c.d}{e}{/if}",
    "{#each xs as x, i (x.id)}{x.name}{i}{/each}",
    "{#await p then v}{v}{:catch e}{e.message}{/await}",
    "{#key k}{@html h}{@render r()}{@const c = d}{c}{/key}",
    "<svelte:element this={tag} class=\"a\">{x}</svelte:element>",
    "<a href=\"/x\"></a><b> </b><i>t</i><u>{v}</u><br /><Foo></Foo><Bar />",
    "{#if (a)}{(b.c)}{/if}{#each (xs) as x}{ x }{/each}{@html h.i}{#key (k)}{k}{/key}",
    "<script lang=\"ts\">let xs: T[];</script>{#each xs as x: T}{x}{/each}{#snippet s(a, b)}{a}{/snippet}",
    "<div on:click={ h } bind:this={ el.x } {...rest} {@attach a.b} class:c={ c }>{@const d = e.f}</div>",
    "<table><tr><td>t</td><td> </td><td></td></tr></table><select><option>o</option></select>",
];

/// Each seed with each insert at each byte position.
#[test]
fn an_insert_at_every_position_formats_identically_without_fused_spans() {
    for seed in SEEDS {
        for at in (0..=seed.len()).filter(|&at| seed.is_char_boundary(at)) {
            for insert in INSERTS {
                let source = format!("{}{insert}{}", &seed[..at], &seed[at..]);
                assert_fused_matches_general(&source, &format_args!("{source:?}"));
            }
        }
    }
}

/// One line of fused constructs walked across the print width a column at a time, plain
/// and with a value — or an attribute name — the plain gate declines (a tab, a wide
/// character, a combining mark after the opening quote, a character that clusters with
/// the byte after it): a fused text is measured as one, so its width has to be the sum the
/// parts measure to at every column.
#[test]
fn a_line_walked_across_the_print_width_formats_identically_without_fused_spans() {
    let values = [
        "v",
        "a\tb",
        "\u{4e2d}\u{6587}",
        "\u{301}x",
        "x\u{600}",
        "\u{1f600}",
    ];
    for value in values {
        for pad in 0..48 {
            let pad = "p".repeat(pad);
            for source in [
                format!(
                    "<div aa=\"{pad}\" bbbbbbbbbb=\"{value}\" cc=\"dd ee\" {{ff}} gg={{hh}} ii={{jj.kk}}>{{ll}}</div>"
                ),
                format!(
                    "<Foo aa=\"{pad}\" bbbbbbbbbb=\"{value}\" class=\"dd ee\" {{ff}} gg={{hh}} ii={{jj.kk}} />"
                ),
                format!(
                    "<Foo aa=\"{pad}\" n{name}={{hh}} m{name}=\"vv\" {{ff}} gg={{hh}} iiiiiiiiiiiiiiii={{jj.kk}} />",
                    name = if value.contains('\t') { "" } else { value }
                ),
                format!(
                    "<p>{pad} {{aaaaaaaaaa}} {{bbbbbbbbbb.cccccccccc}} {value} {{dddddddddd}} {{eeeeeeeeee.ffffffffff}} gggg</p>"
                ),
                format!(
                    "{{#if {pad}aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}}<i title=\"{value}\">{{cccccccccccccccccccc}}</i>{{/if}}"
                ),
            ] {
                assert_fused_matches_general(&source, &format_args!("{source:?}"));
            }
        }
    }
}
