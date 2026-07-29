<!--
	A `{#snippet}` declares a binding and renders nothing, and `clean_nodes` HOISTS it out
	of its fragment before the whitespace rules run — exactly like `{@const}` — so it takes
	its OWN LINE and every authoring converges here. One shape is excluded: a snippet GLUED
	to content on both sides (breaking there WOULD inject a rendered space).
-->
{#if cond}{#snippet fn1()}x{/snippet}{/if}

<!-- A lone snippet in a component -->
<Comp1>{#snippet fn2()}x{/snippet}</Comp1>

<!-- A text sibling: the snippet takes its line, the text keeps its own -->
<div>text1 {#snippet fn3()}x{/snippet}</div>

<!-- Glued to content on ONE side only: the snippet hoists, so the break is render-free -->
<Comp2>text2{#snippet fn4()}x{/snippet}</Comp2>

<!-- GLUED on both sides: `a{#snippet}…{/snippet}b` renders `ab`, so it keeps the author's line -->
<div>a{#snippet fn5()}x{/snippet}b</div>

<!-- A component-hosted glued pair welds the same way (the snippet still becomes a prop) -->
<Comp5>a{#snippet fn11()}x{/snippet}b</Comp5>

<!-- Consecutive snippets each take a line: a hoisted neighbour is not content -->
<Comp3>{#snippet fn6()}x{/snippet}{#snippet fn7()}y{/snippet}text3</Comp3>

<!-- A comment is content: glued on one side only, so the snippet still takes its line -->
<div><!-- c -->{#snippet fn8()}x{/snippet}</div>

<!-- A hoisted non-declaration neighbour ({@debug}) is not content either -->
<Comp4>{@debug cond}{#snippet fn9()}x{/snippet}</Comp4>

<!-- The ROOT fragment is a fragment too: the trailing glue splits there alike -->
{#snippet fn10()}x{/snippet}text4
