<script lang="ts">
	let cond;
	let expr;
	let fn;
</script>

{#if cond}
	<!-- a comma inside the annotation's type arguments is not a second declarator -->
	{@const a1: Map<A, B> = expr}

	<!-- a function type puts an arrow ahead of the declarator's own `=` -->
	{@const a2: (a: A, b: B) => C = expr}

	<!-- the same type arguments on the init side, after `as` and as a call's -->
	{@const a3=expr as Map<A,B>}
	{@const a4=fn<A,B>(expr)}

	<!-- a comment inside a type argument list, on the init side where Svelte reads the
		declarator once -->
	{@const a6=expr as Map<A /* c6 */,B>}

	<!-- a real top-level comma is a second declarator unless the author parenthesized it -->
	{@const a5=(expr,expr)}
{/if}
