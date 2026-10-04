// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A nested `<script>` / `<style>` with a body keeps its own line wherever that line is
//! render-free, and keeps its glue everywhere else.
//!
//! The element renders no box and the compiler keeps it (it is not hoisted), so the whitespace
//! on either side of it stays in one inline formatting context and the compiler's edge trim
//! stops at it. A line break beside it is render-free only where it merges into whitespace that
//! already renders, or where it disappears at the edge of a line box — a block element, or a
//! block parent's content edge. An inline parent's edge, a component's, a block body's inside a
//! line and the root's are not: the line goes on past them. Comments render nothing, so the
//! answer is read through them. A declaration beside such an element answers the way the
//! element does.
//!
//! Not fixtures: several of these cells are render-equivalent to their output, but the fixture
//! validator's render key joins the compiler's separate template chunks with a hole marker, so
//! whitespace never collapses across the nested element there — a one-sided cell, or whitespace
//! the formatter adds between two nested elements, reads as a render change. Each cell pins the
//! exact output and that the output is its own fixed point.

fn format(source: &str) -> String {
    tsv_svelte::format_str(source).expect("svelte format_str")
}

fn assert_formats(source: &str, expected: &str) {
    let out = format(source);
    assert_eq!(out, expected, "first pass of:\n{source}");
    assert_eq!(
        format(&out),
        out,
        "the output must be its own fixed point:\n{out}"
    );
}

/// Glued on the left only.
#[test]
fn glued_left_p() {
    assert_formats(
        r"<p>text1<script>let a = 1;</script> text2</p>",
        r"<p>
	text1
	<script>
		let a = 1;
	</script>
	text2
</p>
",
    );
}

/// Glued on the right only.
#[test]
fn glued_right_p() {
    assert_formats(
        r"<p>text1 <script>let a = 1;</script>text2</p>",
        r"<p>
	text1
	<script>
		let a = 1;
	</script>
	text2
</p>
",
    );
}

/// A script and a style, spaced from each other, each glued to text on its other side.
#[test]
fn run_mid_space_p() {
    assert_formats(
        r"<p>text1<script>let a = 1;</script> <style>div { color: red; }</style>text2</p>",
        r"<p>
	text1
	<script>
		let a = 1;
	</script>
	<style>
		div {
			color: red;
		}
	</style>
	text2
</p>
",
    );
}

/// Glued on the left only.
#[test]
fn glued_left_span() {
    assert_formats(
        r"<span>text1<script>let a = 1;</script> text2</span>",
        r"<span>
	text1
	<script>
		let a = 1;
	</script>
	text2
</span>
",
    );
}

/// Glued on the right only.
#[test]
fn glued_right_span() {
    assert_formats(
        r"<span>text1 <script>let a = 1;</script>text2</span>",
        r"<span>
	text1
	<script>
		let a = 1;
	</script>
	text2
</span>
",
    );
}

/// A script and a style, spaced from each other, each glued to text on its other side.
#[test]
fn run_mid_space_span() {
    assert_formats(
        r"<span>text1<script>let a = 1;</script> <style>div { color: red; }</style>text2</span>",
        r"<span>
	text1
	<script>
		let a = 1;
	</script>
	<style>
		div {
			color: red;
		}
	</style>
	text2
</span>
",
    );
}

/// Glued on the left only.
#[test]
fn glued_left_if() {
    assert_formats(
        r"{#if cond}text1<script>let a = 1;</script> text2{/if}",
        r"{#if cond}
	text1
	<script>
		let a = 1;
	</script>
	text2
{/if}
",
    );
}

/// Glued on the right only.
#[test]
fn glued_right_if() {
    assert_formats(
        r"{#if cond}text1 <script>let a = 1;</script>text2{/if}",
        r"{#if cond}
	text1
	<script>
		let a = 1;
	</script>
	text2
{/if}
",
    );
}

/// A script and a style, spaced from each other, each glued to text on its other side.
#[test]
fn run_mid_space_if() {
    assert_formats(
        r"{#if cond}text1<script>let a = 1;</script> <style>div { color: red; }</style>text2{/if}",
        r"{#if cond}
	text1
	<script>
		let a = 1;
	</script>
	<style>
		div {
			color: red;
		}
	</style>
	text2
{/if}
",
    );
}

/// Glued on the left only.
#[test]
fn glued_left_comp() {
    assert_formats(
        r"<Comp>text1<script>let a = 1;</script> text2</Comp>",
        r"<Comp>
	text1
	<script>
		let a = 1;
	</script>
	text2
</Comp>
",
    );
}

/// Glued on the right only.
#[test]
fn glued_right_comp() {
    assert_formats(
        r"<Comp>text1 <script>let a = 1;</script>text2</Comp>",
        r"<Comp>
	text1
	<script>
		let a = 1;
	</script>
	text2
</Comp>
",
    );
}

/// A script and a style, spaced from each other, each glued to text on its other side.
#[test]
fn run_mid_space_comp() {
    assert_formats(
        r"<Comp>text1<script>let a = 1;</script> <style>div { color: red; }</style>text2</Comp>",
        r"<Comp>
	text1
	<script>
		let a = 1;
	</script>
	<style>
		div {
			color: red;
		}
	</style>
	text2
</Comp>
",
    );
}

/// A comment glued to the element, spaced from the text before it.
#[test]
fn comment_before_spaced() {
    assert_formats(
        r"<p>text1 <!-- c --><script>let a = 1;</script>text2</p>",
        r"<p>
	text1 <!-- c -->
	<script>
		let a = 1;
	</script>
	text2
</p>
",
    );
}

/// A comment glued to the element, spaced from the text after it.
#[test]
fn comment_after_spaced() {
    assert_formats(
        r"<p>text1<script>let a = 1;</script><!-- c --> text2</p>",
        r"<p>
	text1
	<script>
		let a = 1;
	</script>
	<!-- c --> text2
</p>
",
    );
}

/// Comments glued on both sides, at the fragment edges: no content on either side.
#[test]
fn comments_at_both_edges() {
    assert_formats(
        r"<p><!-- c1 --><script>let a = 1;</script><!-- c2 --></p>",
        r"<p>
	<!-- c1 -->
	<script>
		let a = 1;
	</script>
	<!-- c2 -->
</p>
",
    );
}

/// Glued to text before and a block element after: the block boundary is render-free.
#[test]
fn text_then_block() {
    assert_formats(
        r"<div>text1<script>let a = 1;</script><div>text2</div></div>",
        r"<div>
	text1
	<script>
		let a = 1;
	</script>
	<div>text2</div>
</div>
",
    );
}

/// Inside `<svelte:head>` the hoisted `<title>` is no neighbour, so the JSON-LD script is glued
/// on one side only.
#[test]
fn head_json_ld_one_line() {
    assert_formats(
        r#"<svelte:head><title>text</title><script type="application/ld+json">{ "a": 1 }</script><meta name="value1" content="value2" /></svelte:head>"#,
        r#"<svelte:head>
	<title>text</title>
	<script type="application/ld+json">
{ "a": 1 }
	</script>
	<meta name="value1" content="value2" />
</svelte:head>
"#,
    );
}

/// Inside `<svelte:head>` between two glued `<meta>`s: glued on both sides, so it lays out as a
/// glued inline element does.
#[test]
fn head_glued_between_metas() {
    assert_formats(
        r#"<svelte:head><meta name="value1" /><script>let a = 1;</script><meta name="value2" /></svelte:head>"#,
        r#"<svelte:head>
	<meta name="value1" /><script>
		let a = 1;
	</script><meta name="value2" />
</svelte:head>
"#,
    );
}

/// A block parent's content edge is a line-box edge, so a break there is not rendered.
#[test]
fn block_parent_edge_keeps_own_line() {
    assert_formats(
        r"<p>text1<script>let a = 1;</script></p>",
        r"<p>
	text1
	<script>
		let a = 1;
	</script>
</p>
",
    );
}

/// A block body whose tag sits at a block parent's edge reads that edge as its own.
#[test]
fn block_body_in_block_context_keeps_own_line() {
    assert_formats(
        r"<div>{#if cond}text1<script>let a = 1;</script>{/if}</div>",
        r"<div>
	{#if cond}
		text1
		<script>
			let a = 1;
		</script>
	{/if}
</div>
",
    );
}

/// The root's edge is not a line-box edge (a component can render inside a line), so a block
/// body at the root keeps the glue.
#[test]
fn block_body_at_root_keeps_glue() {
    assert_formats(
        r"{#if cond}text1<script>let a = 1;</script>{/if}",
        r"{#if cond}
	text1<script>
		let a = 1;
	</script>
{/if}
",
    );
}

/// A declaration beside an element that takes its own line answers as beside a block.
#[test]
fn comment_and_declaration_after_element_in_block_parent() {
    assert_formats(
        r"<p><!-- c --><script>let a = 1;</script>{const q = 2}a</p>",
        r"<p>
	<!-- c -->
	<script>
		let a = 1;
	</script>
	{const q = 2}
	a
</p>
",
    );
}

/// A declaration before an element that takes its own line, a spaced comment after it.
#[test]
fn declaration_before_element_comment_after() {
    assert_formats(
        r"<p>a{const q = 2}<script>let a = 1;</script><!-- c --> b</p>",
        r"<p>
	a
	{const q = 2}
	<script>
		let a = 1;
	</script>
	<!-- c --> b
</p>
",
    );
}

/// A declaration beside an element that keeps its glue keeps its glue too.
#[test]
fn comment_before_element_declaration_after_in_block_body() {
    assert_formats(
        r"{#if c}<!-- c --><script>let a = 1;</script>{@const q = 2}a{/if}",
        r"{#if c}
	<!-- c --><script>
		let a = 1;
	</script>{@const q = 2}a
{/if}
",
    );
}

/// The same inside a line: the element and the declaration agree.
#[test]
fn declaration_after_glued_element_in_inline_block_body() {
    assert_formats(
        r"<p>x{#if c}<script>let a = 1;</script>{@const k = 1}bb{/if}y</p>",
        r"<p>
	x{#if c}
		<script>
			let a = 1;
		</script>{@const k = 1}bb
	{/if}y
</p>
",
    );
}

/// A script and a style glued together inside a block parent each take their own line: the breaks
/// at the block's content edges disappear, and the one between them shares its line with nothing
/// that renders.
#[test]
fn glued_pair_in_block_parent_takes_own_lines() {
    assert_formats(
        r"<div><script>let a = 1;</script><style>.a { color: red; }</style></div>",
        r"<div>
	<script>
		let a = 1;
	</script>
	<style>
		.a {
			color: red;
		}
	</style>
</div>
",
    );
}
