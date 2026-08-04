<script lang="ts">
	let cond;
	let expr;
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
{/if}
