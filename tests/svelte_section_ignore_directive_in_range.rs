// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A `<!-- prettier-ignore -->` directly above a hoisted `<script>` / `<style>` freezes that
//! section even when both sit inside a `format-ignore` range.
//!
//! A comment inside a frozen range never travels with a section (the range's verbatim slice
//! prints it where it stands), so the section's own travelling run does not hold the directive
//! here; the freeze reads the nearest node above the section instead, provided no other section
//! stands between them — which is what keeps one section's directive from freezing the next.
//!
//! Not a fixture: the output is not its own fixed point. The section moves out of the range
//! while the directive stays inside it, so the next pass finds the section with no directive
//! above it and formats it. Prettier moves the directive out with the section; tsv keeps a
//! comment inside a range where it was written. What this pins is the first pass: the section
//! prints as written, as prettier prints it.

fn format(source: &str) -> String {
    tsv_svelte::format_str(source).expect("svelte format_str")
}

#[test]
fn directive_inside_a_range_freezes_the_section_below_it() {
    let source = "<p>a</p>\n<!-- prettier-ignore-start -->\n<!-- prettier-ignore -->\n\
                  <script>\nlet   i\n</script>\n<!-- prettier-ignore-end -->\n<p>b</p>\n";
    let out = format(source);
    assert!(
        out.starts_with("<script>\nlet   i\n</script>\n"),
        "the frozen script must print as written:\n{out}"
    );
}

#[test]
fn directive_inside_a_range_does_not_reach_past_another_section() {
    let source = "<p>a</p>\n<!-- prettier-ignore-start -->\n<!-- prettier-ignore -->\n\
                  <style>\np {  color: red  }\n</style>\n<script>\nlet   i\n</script>\n\
                  <!-- prettier-ignore-end -->\n<p>b</p>\n";
    let out = format(source);
    assert!(
        out.starts_with("<script>\n\tlet i;\n</script>\n"),
        "the script below the frozen style must be formatted:\n{out}"
    );
    assert!(
        out.contains("<style>\np {  color: red  }\n</style>"),
        "the style must print as written:\n{out}"
    );
}
