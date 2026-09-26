<!--
	A nested `<script>` / `<style>` glued to content on both sides keeps both glued boundaries in
	every fragment. Its body breaks onto its own lines, so the fragment around it lays out
	block-style, and a space in front of the glued run is where the run's line starts.
-->

<!-- inline element parent -->
<span>
	text1<script>
		let a = 1;
	</script>text2
</span>

<!-- component parent, expression tags on both sides -->
<Comp>
	{expr1}<style>
		div {
			color: red;
		}
	</style>{expr2}
</Comp>

<!-- block bodies -->
{#if cond}
	text1<script>
		let b = 2;
	</script>text2
{:else}
	text3<style>
		div {
			color: red;
		}
	</style>text4
{/if}

{#each items as item}
	<b>inline1</b><script>
		let c = 3;
	</script><i>inline2</i>
{/each}

<!-- a comment and a declaration tag inside the glued run -->
{#snippet fn()}
	text1<!-- c --><script>
		let d = 4;
	</script>text2
{/snippet}

{#if cond}
	text1{@const e = 5}<script>
		let f = 6;
	</script>text2
{/if}

<!-- a word in front of the glued run -->
<p>
	text1
	text2<script>
		let g = 7;
	</script>text3 text4
</p>
