<script lang="ts">
	let cond;
	let expr;
	// the null control for a6 below — the same comment at the same union seam, in a
	// context Svelte reads once: one copy, attached to `A` alone by both parsers
	let a0: A /* c0 */ | B;
</script>

{#if cond}
	<!-- a comment inside the binding's type annotation -->
	{@const a1: /* c1 */ T = expr}

	<!-- the same, on a destructuring binding -->
	{@const { a2 }: /* c2 */ T = expr}

	<!-- an init comment, with an annotation present on the binding -->
	{@const a3: T = /* c3 */ expr}

	<!-- an init comment with no annotation at all -->
	{@const a4 = /* c4 */ expr}
	<!-- the dedent reads the source acorn was handed, and for an annotation Svelte blanks
		the prefix to spaces, so the tab opening the line below is not indentation it can see -->
	{@const a5: /*
	 c5 */ T = expr}

	<!-- at a union seam the two copies land on DIFFERENT nodes, so the duplication is
		an attachment tsv has nowhere rather than a longer list: canonical trails `A`
		with one copy and leads `B` with the other, where a1's single node takes both -->
	{@const a6: A /* c6 */ | B = expr}
{/if}
