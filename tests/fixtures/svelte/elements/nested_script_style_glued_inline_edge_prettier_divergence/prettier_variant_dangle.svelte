<!--
	A nested `<script>` / `<style>` renders no box, so the whitespace beside it only disappears at
	the edge of a line: at the content edge of an INLINE parent the text on the other side of the
	parent continues the line. Glued to content on one side and to such an edge on the other, it
	keeps its glue, and so does a pair of them.
-->

<!-- glued to text before, the inline parent's end after -->
<p>
	text1<span
		>text2<script>
			let a = 1;
		</script></span
	>text3
</p>

<!-- the inline parent's start before, glued to text after -->
<p>
	text1<span
		><script>
			let b = 2;
		</script>text2</span
	>text3
</p>

<!-- a comment between the start and the element renders nothing -->
<p>
	text1<b
		><!-- c --><script>
			let c = 3;
		</script>text2</b
	>
</p>

<!-- two elements glued together, both edges inline -->
<p>
	text1<span
		><script>
			let d = 4;
		</script><style>
			div {
				color: red;
			}
		</style></span
	>text2
</p>

<!-- a component parent -->
<p>
	text1<Comp
		>text2<script>
			let e = 5;
		</script></Comp
	>
</p>

<!-- a block body inside a line -->
<p>
	text1{#if cond}text2<script>
			let f = 6;
		</script>{/if}text3
</p>

<!-- whitespace that reaches only the inline parent's end is trimmed there -->
<p>
	text1<span
		>text2<script>
			let g = 7;
		</script>
	</span>text3
</p>
