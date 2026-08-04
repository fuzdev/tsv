<script lang="ts">
	let cond;
	let expr;
</script>

{#if cond}
	<!-- a type annotation on the binding, which Svelte reads only in a TS component -->
	{@const a1: T = expr}
	{@const a2: A | B = expr}

	<!-- a destructuring pattern carries its annotation as a sibling of the binding -->
	{@const { a3 }: T = expr}
	{@const [a4]: T = expr}
{/if}
