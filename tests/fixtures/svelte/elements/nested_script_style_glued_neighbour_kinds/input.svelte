<!--
	A nested `<script>` glued on both sides keeps both glued boundaries whatever renders beside it:
	every neighbour below meets the other directly, so a break on either side of the element would
	render a space between them.
-->

<!-- if blocks on both sides -->
<div>
	{#if cond1}text1{/if}<script>
		let a = 1;
	</script>{#if cond2}text2{/if}
</div>

<!-- html tags on both sides -->
<div>
	{@html expr1}<script>
		let b = 2;
	</script>{@html expr2}
</div>

<!-- render tags on both sides -->
<div>
	{@render fn1()}<script>
		let c = 3;
	</script>{@render fn2()}
</div>

<!-- a component after -->
<div>
	text1<script>
		let d = 4;
	</script><Comp />
</div>

<!-- a svelte:element before -->
<div>
	<svelte:element this={tag}>text1</svelte:element><script>
		let e = 5;
	</script>text2
</div>

<!-- a textarea before -->
<div>
	<textarea>text1</textarea><script>
		let f = 6;
	</script>text2
</div>

<!-- a comment after, inside the glued run -->
<div>
	text1<script>
		let g = 7;
	</script><!-- c -->text2
</div>
