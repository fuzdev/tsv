<script lang="ts">
	let cond;
	let expr;
</script>

{#if cond}
	<!-- a type annotation on the binding -->
	{@const a1: T = expr}

	<!-- a destructuring binding carries its annotation as a sibling of the pattern -->
	{@const { a2 }: T = expr}

	<!-- an annotation that is a whole function type -->
	{@const a3: (a: A, b: B) => C = expr}

	<!-- the null control: the same tag with no annotation -->
	{@const a4 = expr}
{/if}
